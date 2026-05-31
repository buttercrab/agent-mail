#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${AGENT_MAIL_URL:-https://agent-mail.cc}"
TOKEN="${AGENT_MAIL_TOKEN:?AGENT_MAIL_TOKEN is required}"
TMPDIR="$(mktemp -d /tmp/agent-mail-notify-smoke-XXXXXX)"
PROJECT="notify-smoke-$(date +%Y%m%d%H%M%S)-$$"
RECEIVER_IDENTITY="notify-receiver-$$"
RECEIVER_ROLE="notify-reviewer-$$"
SENDER_IDENTITY="notify-sender-$$"
SENDER_ROLE="notify-sender-role-$$"

cleanup() {
  local exit_status=$?
  if [[ -n "${WAIT_PID:-}" ]]; then
    kill "$WAIT_PID" 2>/dev/null || true
    wait "$WAIT_PID" 2>/dev/null || true
  fi
  if [[ $exit_status -eq 0 ]]; then
    rm -rf "$TMPDIR"
  else
    echo "tmpdir: $TMPDIR" >&2
    find "$TMPDIR" -maxdepth 1 -type f -print -exec sed -n '1,220p' {} \; >&2 || true
  fi
}
trap cleanup EXIT

post_json() {
  local path="$1"
  local body="$2"
  curl -fsS -X POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    --data "$body" \
    "$BASE_URL$path"
}

assert_json_file() {
  local path="$1"
  local expr="$2"
  python3 - "$path" "$expr" <<'PY'
import json
import sys

path, expr = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as file:
    data = json.load(file)
if not eval(expr, {"json": json}, {"data": data}):
    raise SystemExit(f"assertion failed: {expr}\n{json.dumps(data, indent=2)}")
PY
}

wait_for_file_json() {
  local path="$1"
  local expr="$2"
  for _ in {1..160}; do
    if [[ -s "$path" ]] && python3 - "$path" "$expr" <<'PY'
import json
import sys

path, expr = sys.argv[1], sys.argv[2]
try:
    with open(path, encoding="utf-8") as file:
        data = json.load(file)
except Exception:
    raise SystemExit(1)
if eval(expr, {"json": json}, {"data": data}):
    raise SystemExit(0)
raise SystemExit(1)
PY
    then
      return 0
    fi
    sleep 0.25
  done
  echo "timed out waiting for $path to satisfy $expr" >&2
  return 1
}

health_json="$(curl -fsS "$BASE_URL/health")"
printf '%s' "$health_json" >"$TMPDIR/health.json"
assert_json_file "$TMPDIR/health.json" 'data["ok"] is True'

post_json /v1/participants/start "$(python3 -c 'import json, sys; print(json.dumps({"identity":sys.argv[1],"role":sys.argv[2]}))' "$RECEIVER_IDENTITY" "$RECEIVER_ROLE")" >/dev/null
post_json /v1/participants/start "$(python3 -c 'import json, sys; print(json.dumps({"identity":sys.argv[1],"role":sys.argv[2]}))' "$SENDER_IDENTITY" "$SENDER_ROLE")" >/dev/null
post_json /v1/projects "$(python3 -c 'import json, sys; print(json.dumps({"alias":sys.argv[1],"root":"/notify/adapter/smoke"}))' "$PROJECT")" >/dev/null

cargo run -q -p agent-mail-notify -- codex-wait \
  --url "$BASE_URL" \
  --project "$PROJECT" \
  --identity "$RECEIVER_IDENTITY" \
  --timeout-seconds 40 >"$TMPDIR/codex-event.json" 2>"$TMPDIR/codex-event.err" &
WAIT_PID=$!

sleep 1

post_json /v1/messages "$(python3 -c 'import json, sys; print(json.dumps({"sender_identity":sys.argv[1],"project":sys.argv[2],"to_kind":"role","to":sys.argv[3],"subject":"Notify adapter smoke","body":"real notify adapter body"}))' "$SENDER_IDENTITY" "$PROJECT" "$RECEIVER_ROLE")" >"$TMPDIR/message.json"

wait "$WAIT_PID"
WAIT_PID=""

wait_for_file_json "$TMPDIR/codex-event.json" 'data["kind"] == "inbox_updated" and data["resource"]["unread_count"] >= 1'

# claude-channel-serve is a persistent stdio MCP server, not a one-shot. Drive
# it by feeding a single `initialize` request on its stdin and capturing the
# first stdout line. It exits on stdin EOF, so closing the pipe ends it; a
# bounded timeout guards against a hang. Assert the handshake declares the
# claude/channel capability.
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}' \
  | cargo run -q -p agent-mail-notify -- claude-channel-serve \
      --url "$BASE_URL" \
      --project "$PROJECT" \
      --identity "$RECEIVER_IDENTITY" \
      2>"$TMPDIR/claude-channel.err" \
  | head -n 1 >"$TMPDIR/claude-channel.json"

wait_for_file_json "$TMPDIR/claude-channel.json" 'data["result"]["capabilities"]["experimental"]["claude/channel"] == {}'

echo "notify adapter smoke passed"
echo "project=$PROJECT"
echo "receiver=$RECEIVER_IDENTITY"
