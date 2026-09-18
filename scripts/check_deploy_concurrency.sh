#!/usr/bin/env bash
# The deploy workflows must not serialize the whole run behind a workflow-level
# concurrency group. While a run holds such a group, GitHub cancels every later
# pending run, so a single wedged run silently drops all subsequent dispatches.
# The guard is scoped to the deploy job and the job is bounded, so a stuck run
# releases the group instead of absorbing later dispatches.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fail() {
  printf 'deploy-concurrency: %s\n' "$*" >&2
  exit 1
}

for workflow in production staging; do
  file="$ROOT/.github/workflows/${workflow}.yml"
  [ -f "$file" ] || fail "${workflow}.yml is missing"

  if grep -qE '^concurrency:' "$file"; then
    fail "${workflow}.yml uses a workflow-level concurrency group"
  fi
  grep -qE '^    concurrency:$' "$file" \
    || fail "${workflow}.yml lost its job-scoped concurrency guard"
  grep -qE '^      group: .+$' "$file" \
    || fail "${workflow}.yml lost its concurrency group name"
  grep -qE '^      cancel-in-progress: (true|false)$' "$file" \
    || fail "${workflow}.yml lost its explicit cancel-in-progress intent"
  grep -qE '^    timeout-minutes: ' "$file" \
    || fail "${workflow}.yml lost its bounded job timeout"
done
