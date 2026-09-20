#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKFLOWS="${1:-$ROOT/.github/workflows}"

fail() { printf 'deploy-concurrency: %s\n' "$*" >&2; exit 1; }
for environment in production staging; do
  file="$WORKFLOWS/$environment.yml"
  job="deploy-$environment"
  [[ -f "$file" ]] || fail "$environment.yml is missing"
  ! grep -qE '^concurrency:' "$file" || fail "workflow-level concurrency gates non-deploy jobs"
  # Scope every assertion to this job and its concurrency mapping, so another
  # job's valid configuration cannot hide a missing guard.
  body="$(awk -v job="$job" '
    $0 == "  " job ":" { found = 1; next }
    found && /^  [^ ]/ { exit }
    found { print }
  ' "$file")"
  [[ -n "$body" ]] || fail "missing job $job"
  [[ "$(grep -cE '^    concurrency:$' <<< "$body")" == 1 ]] \
    || fail "$job needs one concurrency mapping"
  grep -qE '^    timeout-minutes: [1-9][0-9]*$' <<< "$body" \
    || fail "$job needs a positive timeout"
  guard="$(awk '
    /^    concurrency:$/ { found = 1; next }
    found && /^    [^ ]/ { exit }
    found { print }
  ' <<< "$body")"
  grep -qx "      group: $environment" <<< "$guard" \
    || fail "$job must use its environment concurrency group"
  grep -qx '      cancel-in-progress: false' <<< "$guard" \
    || fail "$job must not cancel an active deployment"
  grep -qx '      queue: max' <<< "$guard" \
    || fail "$job must queue pending deployments instead of replacing them"
done
