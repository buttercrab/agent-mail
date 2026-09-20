#!/usr/bin/env python3
"""Reject deployment guards that lose serialization, queueing, or time bounds."""

from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts" / 'check_deploy_concurrency.sh'
TARGETS = [('production.yml', 'deploy-production', 'production'), ('staging.yml', 'deploy-staging', 'staging')]


class DeploymentConcurrencyTests(unittest.TestCase):
    def setUp(self):
        self.workflows = {
            name: (ROOT / ".github" / "workflows" / name).read_text()
            for name, _, _ in TARGETS
        }

    def check(self, workflows):
        with tempfile.TemporaryDirectory() as directory:
            for name, contents in workflows.items():
                (Path(directory) / name).write_text(contents)
            return subprocess.run(
                ["bash", str(CHECKER), directory], capture_output=True, text=True,
                timeout=10,
            )

    def mutate_job(self, name, job, transform):
        source = self.workflows[name]
        match = re.search(r"^  " + re.escape(job) + r":\n.*?(?=^  \S|\Z)",
                          source, re.MULTILINE | re.DOTALL)
        self.assertIsNotNone(match)
        changed = transform(match.group())
        self.assertNotEqual(changed, match.group())
        return {**self.workflows, name: source[:match.start()] + changed + source[match.end():]}

    def test_repository_workflows_satisfy_the_contract(self):
        result = self.check(self.workflows)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_every_mutating_job_needs_its_own_complete_guard(self):
        for name, job, group in TARGETS:
            mutations = {
                "missing queue": lambda s: s.replace("      queue: max\n", "", 1),
                "wrong group": lambda s: s.replace("      group: " + group + "\n",
                                                     "      group: unrelated\n", 1),
                "cancels active deployment": lambda s: s.replace(
                    "      cancel-in-progress: false\n", "      cancel-in-progress: true\n", 1),
                "missing mapping": lambda s: s.replace("    concurrency:\n", "", 1),
                "zero timeout": lambda s: re.sub(r"^    timeout-minutes: \d+$",
                                                   "    timeout-minutes: 0", s, count=1, flags=re.MULTILINE),
                "missing timeout": lambda s: re.sub(r"^    timeout-minutes: \d+\n",
                                                      "", s, count=1, flags=re.MULTILINE),
                "missing job": lambda s: "",
            }
            for issue, mutate in mutations.items():
                with self.subTest(job=job, issue=issue):
                    result = self.check(self.mutate_job(name, job, mutate))
                    self.assertNotEqual(result.returncode, 0, issue)


if __name__ == "__main__":
    unittest.main()
