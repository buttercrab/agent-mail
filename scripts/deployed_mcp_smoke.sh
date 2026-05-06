#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${AGENT_MAIL_URL:?AGENT_MAIL_URL is required}"
TOKEN="${AGENT_MAIL_TOKEN:?AGENT_MAIL_TOKEN is required}"
PUBLIC_IP="${PUBLIC_IP:?PUBLIC_IP is required to verify the private service port is not exposed}"
PRIVATE_PORT="${PRIVATE_PORT:-8787}"
EXPECTED_AGENT_MAIL_HOST="${EXPECTED_AGENT_MAIL_HOST:-}"
EXPECTED_ENVIRONMENT="${EXPECTED_ENVIRONMENT:-}"
TMPDIR="$(mktemp -d /tmp/agent-mail-public-mcp-XXXXXX)"
PROJECT="public-mcp-$(date +%Y%m%d%H%M%S)-$$"
REVIEWER_ROLE="public-smoke-reviewer-$$"
SENDER_ROLE="public-smoke-sender-$$"

cleanup() {
  local status=$?
  if [[ -n "${SSE_PID:-}" ]]; then
    kill "$SSE_PID" 2>/dev/null || true
    wait "$SSE_PID" 2>/dev/null || true
  fi
  if [[ $status -eq 0 ]]; then
    rm -rf "$TMPDIR"
  else
    echo "tmpdir: $TMPDIR" >&2
    [[ -f "$TMPDIR/sse.log" ]] && cat "$TMPDIR/sse.log" >&2
  fi
}
trap cleanup EXIT

assert_json() {
  python3 -c 'import json, sys
data = json.load(sys.stdin)
expr = sys.argv[1]
if not eval(expr, {"json": json}, {"data": data}):
    raise SystemExit(f"assertion failed: {expr}\n{json.dumps(data, indent=2)}")' "$1"
}

if [[ -n "$EXPECTED_AGENT_MAIL_HOST" ]]; then
  actual_host="$(python3 -c 'from urllib.parse import urlparse; import sys; print(urlparse(sys.argv[1]).hostname or "")' "$BASE_URL")"
  if [[ "$actual_host" != "$EXPECTED_AGENT_MAIL_HOST" ]]; then
    echo "expected AGENT_MAIL_URL host $EXPECTED_AGENT_MAIL_HOST, got $actual_host" >&2
    exit 1
  fi
fi

mcp_request() {
  python3 -c 'import json, sys
method = sys.argv[1]
request = {"jsonrpc": "2.0", "method": method}
if sys.argv[2]:
    request["id"] = int(sys.argv[2])
if sys.argv[3]:
    request["params"] = json.loads(sys.argv[3])
print(json.dumps(request))' "$@"
}

wait_for_sse_uri() {
  local uri="$1"
  for _ in {1..160}; do
    if python3 - "$TMPDIR/sse.log" "$uri" <<'PY'
import json
import sys

path, expected = sys.argv[1], sys.argv[2]
try:
    lines = open(path, encoding="utf-8").read().splitlines()
except FileNotFoundError:
    raise SystemExit(1)

for line in lines:
    if not line.startswith("data:"):
        continue
    payload = line.removeprefix("data:").strip()
    if not payload:
        continue
    try:
        message = json.loads(payload)
    except json.JSONDecodeError:
        continue
    if (
        message.get("method") == "notifications/resources/updated"
        and message.get("params", {}).get("uri") == expected
    ):
        raise SystemExit(0)
raise SystemExit(1)
PY
    then
      return 0
    fi
    sleep 0.25
  done
  echo "missing SSE resource update for $uri" >&2
  return 1
}

wait_for_sse_method() {
  local method="$1"
  for _ in {1..160}; do
    if python3 - "$TMPDIR/sse.log" "$method" <<'PY'
import json
import sys

path, expected = sys.argv[1], sys.argv[2]
try:
    lines = open(path, encoding="utf-8").read().splitlines()
except FileNotFoundError:
    raise SystemExit(1)

for line in lines:
    if not line.startswith("data:"):
        continue
    payload = line.removeprefix("data:").strip()
    if not payload:
        continue
    try:
        message = json.loads(payload)
    except json.JSONDecodeError:
        continue
    if message.get("method") == expected:
        raise SystemExit(0)
raise SystemExit(1)
PY
    then
      return 0
    fi
    sleep 0.25
  done
  echo "missing SSE method $method" >&2
  return 1
}

mcp_post() {
  local session="$1"
  local body="$2"
  local out="$TMPDIR/body.json"
  local headers="$TMPDIR/headers.txt"
  local extra_headers=(
    -H "Authorization: Bearer $TOKEN"
    -H "Content-Type: application/json"
    -H "Accept: application/json, text/event-stream"
    -H "Origin: $BASE_URL"
  )
  if [[ -n "$session" ]]; then
    extra_headers+=(
      -H "MCP-Session-Id: $session"
      -H "MCP-Protocol-Version: 2025-11-25"
    )
  fi
  curl -fsS -D "$headers" -o "$out" -X POST "${extra_headers[@]}" --data "$body" "$BASE_URL/mcp"
  cat "$out"
}

mcp_init() {
  local out="$TMPDIR/init-body.json"
  local headers="$TMPDIR/init-headers.txt"
  curl -fsS -D "$headers" -o "$out" -X POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -H "Origin: $BASE_URL" \
    --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"agent-mail-public-smoke","version":"0"}}}' \
    "$BASE_URL/mcp" >/dev/null
  assert_json 'data["result"]["protocolVersion"] == "2025-11-25" and data["result"]["capabilities"]["resources"]["subscribe"] is True' <"$out"
  awk 'tolower($1)=="mcp-session-id:" {gsub("\r","",$2); print $2}' "$headers"
}

tool_call() {
  local session="$1"
  local id="$2"
  local name="$3"
  local args="$4"
  mcp_post "$session" "$(python3 -c 'import json, sys
print(json.dumps({"jsonrpc":"2.0","id":int(sys.argv[1]),"method":"tools/call","params":{"name":sys.argv[2],"arguments":json.loads(sys.argv[3])}}))' "$id" "$name" "$args")"
}

health_json="$(curl -fsS "$BASE_URL/health")"
printf '%s' "$health_json" | assert_json 'data["ok"] is True'
if [[ -n "$EXPECTED_ENVIRONMENT" ]]; then
  printf '%s' "$health_json" | assert_json 'data.get("environment") == "'"$EXPECTED_ENVIRONMENT"'"'
fi

unauth_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' "$BASE_URL/mcp")"
[[ "$unauth_status" == "401" ]]
bad_origin_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST -H "Authorization: Bearer $TOKEN" -H "Origin: https://evil.example" -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' "$BASE_URL/mcp")"
[[ "$bad_origin_status" == "403" ]]

receiver_session="$(mcp_init)"
sender_session="$(mcp_init)"
[[ -n "$receiver_session" && -n "$sender_session" && "$receiver_session" != "$sender_session" ]]

mcp_post "$receiver_session" '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null
mcp_post "$sender_session" '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null

receiver_start="$(tool_call "$receiver_session" 2 agent_mail_start "$(python3 -c 'import json, sys; print(json.dumps({"role":sys.argv[1]}))' "$REVIEWER_ROLE")")"
receiver_identity="$(printf '%s' "$receiver_start" | python3 -c 'import json, sys; print(json.loads(json.load(sys.stdin)["result"]["content"][0]["text"])["identity"])')"
tool_call "$sender_session" 3 agent_mail_start "$(python3 -c 'import json, sys; print(json.dumps({"role":sys.argv[1]}))' "$SENDER_ROLE")" >/dev/null

curl -fsS -N \
  -H "Authorization: Bearer $TOKEN" \
  -H "Accept: text/event-stream" \
  -H "MCP-Session-Id: $receiver_session" \
  -H "MCP-Protocol-Version: 2025-11-25" \
  -H "Origin: $BASE_URL" \
  "$BASE_URL/mcp" >"$TMPDIR/sse.log" &
SSE_PID=$!
sleep 0.5

tool_call "$sender_session" 4 agent_mail_project_add "$(python3 -c 'import json, sys; print(json.dumps({"alias":sys.argv[1],"root":"/public/mcp/smoke"}))' "$PROJECT")" >/dev/null
wait_for_sse_method "notifications/resources/list_changed"

inbox_uri="agent-mail://projects/$PROJECT/inbox?identity=$receiver_identity"
mcp_post "$receiver_session" "$(mcp_request resources/list 40 '{}')" | assert_json 'any(item["uri"] == "'"$inbox_uri"'" for item in data["result"]["resources"])'
mcp_post "$receiver_session" "$(mcp_request resources/subscribe 5 "$(python3 -c 'import json, sys; print(json.dumps({"uri":sys.argv[1]}))' "$inbox_uri")")" | assert_json 'data["result"] == {}'

tool_call "$sender_session" 6 agent_mail_send "$(python3 -c 'import json, sys; print(json.dumps({"project":sys.argv[1],"to":sys.argv[2],"subject":"Public MCP smoke","body":"real public mcp body"}))' "$PROJECT" "$REVIEWER_ROLE")" >/dev/null
wait_for_sse_uri "$inbox_uri"

inbox_json="$(mcp_post "$receiver_session" "$(mcp_request resources/read 7 "$(python3 -c 'import json, sys; print(json.dumps({"uri":sys.argv[1]}))' "$inbox_uri")")")"
printf '%s' "$inbox_json" | assert_json 'json.loads(data["result"]["contents"][0]["text"])["unread_count"] == 1'
mail_id="$(printf '%s' "$inbox_json" | python3 -c 'import json, sys; data=json.loads(json.load(sys.stdin)["result"]["contents"][0]["text"]); print(data["messages"][0]["id"])')"

message_uri="agent-mail://projects/$PROJECT/messages/$mail_id?identity=$receiver_identity"
mcp_post "$receiver_session" "$(mcp_request resources/subscribe 8 "$(python3 -c 'import json, sys; print(json.dumps({"uri":sys.argv[1]}))' "$message_uri")")" | assert_json 'data["result"] == {}'
mcp_post "$receiver_session" "$(mcp_request resources/read 9 "$(python3 -c 'import json, sys; print(json.dumps({"uri":sys.argv[1]}))' "$message_uri")")" | assert_json 'json.loads(data["result"]["contents"][0]["text"])["body"] == "real public mcp body"'

tool_call "$receiver_session" 10 agent_mail_mark_read "$(python3 -c 'import json, sys; print(json.dumps({"project":sys.argv[1],"mail_id":sys.argv[2]}))' "$PROJECT" "$mail_id")" >/dev/null
wait_for_sse_uri "$message_uri"
mcp_post "$receiver_session" "$(mcp_request resources/read 11 "$(python3 -c 'import json, sys; print(json.dumps({"uri":sys.argv[1]}))' "$inbox_uri")")" | assert_json 'json.loads(data["result"]["contents"][0]["text"])["unread_count"] == 0'

if python3 - "$PUBLIC_IP" "$PRIVATE_PORT" <<'PY'
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
try:
    with socket.create_connection((host, port), timeout=3):
        raise SystemExit(0)
except OSError:
    raise SystemExit(1)
PY
then
  echo "raw private service port is publicly reachable: $PUBLIC_IP:$PRIVATE_PORT" >&2
  exit 1
fi

echo "deployed mcp smoke passed"
echo "project=$PROJECT"
echo "receiver=$receiver_identity"
echo "mail_id=$mail_id"
