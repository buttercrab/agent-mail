#!/usr/bin/env bash
set -euo pipefail

# Public liveness probe for the Agent Mail edge.
#
# The only deployed check (scripts/deployed_mcp_smoke.sh) runs inside the manual
# Production Deploy workflow, so an origin outage that leaves Cloudflare
# returning 522 goes unnoticed until the next manual deploy. This probe reads the
# unauthenticated /health endpoint, needs no secrets, and is safe to run on a
# schedule.

PRODUCTION_HEALTH_URL="${PRODUCTION_HEALTH_URL:-https://agent-mail.cc/health}"
PRODUCTION_ENVIRONMENT="${PRODUCTION_ENVIRONMENT:-production}"
STAGING_HEALTH_URL="${STAGING_HEALTH_URL:-https://staging.agent-mail.cc/health}"
STAGING_ENVIRONMENT="${STAGING_ENVIRONMENT:-staging}"

probe() {
  local label="$1"
  local url="$2"
  local expected_environment="$3"

  local body
  body="$(mktemp)"

  local status
  status="$(curl -sS --max-time 20 -o "$body" -w '%{http_code}' "$url" || true)"

  if [[ "$status" != "200" ]]; then
    echo "FAIL $label: GET $url returned HTTP $status" >&2
    sed -n '1,20p' "$body" >&2 || true
    rm -f "$body"
    return 1
  fi

  if ! python3 - "$expected_environment" "$label" "$body" <<'PY'
import json
import sys

expected_environment, label, path = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, encoding="utf-8") as handle:
    try:
        data = json.load(handle)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{label}: health response is not JSON: {exc}")

if data.get("ok") is not True:
    raise SystemExit(f"{label}: health response did not report ok=true: {data!r}")
actual_environment = data.get("environment")
if actual_environment != expected_environment:
    raise SystemExit(
        f"{label}: health response reported environment={actual_environment!r}, "
        f"expected {expected_environment!r}"
    )
PY
  then
    rm -f "$body"
    return 1
  fi

  rm -f "$body"
  echo "ok $label: $url (environment=$expected_environment)"
}

result=0
probe "production" "$PRODUCTION_HEALTH_URL" "$PRODUCTION_ENVIRONMENT" || result=1
probe "staging" "$STAGING_HEALTH_URL" "$STAGING_ENVIRONMENT" || result=1

if [[ "$result" -ne 0 ]]; then
  echo "public health check failed" >&2
  exit 1
fi

echo "public health check passed"
