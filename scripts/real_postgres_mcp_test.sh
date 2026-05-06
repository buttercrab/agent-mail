#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPDIR="$(mktemp -d /tmp/agent-mail-mcp-test-XXXXXX)"
PGDATA="$TMPDIR/pgdata"
PGLOG="$TMPDIR/postgres.log"
SERVERLOG="$TMPDIR/server.log"
TOKEN="mcp-test-token"
POSTGRES_BIN="${POSTGRES_BIN:-}"

find_postgres_bin() {
  if [[ -n "$POSTGRES_BIN" && -x "$POSTGRES_BIN/initdb" ]]; then
    return 0
  fi
  local candidate
  for candidate in \
    /opt/homebrew/opt/postgresql@17/bin \
    /opt/homebrew/opt/postgresql@16/bin \
    /usr/local/opt/postgresql@17/bin \
    /usr/local/opt/postgresql@16/bin \
    /usr/lib/postgresql/*/bin; do
    if [[ -x "$candidate/initdb" ]]; then
      POSTGRES_BIN="$candidate"
      return 0
    fi
  done
  echo "could not find PostgreSQL binaries; set POSTGRES_BIN" >&2
  exit 1
}

cleanup() {
  local status=$?
  if [[ -n "${SSE_PID:-}" ]]; then
    kill "$SSE_PID" 2>/dev/null || true
    wait "$SSE_PID" 2>/dev/null || true
  fi
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  if [[ -n "${POSTGRES_PID:-}" ]]; then
    "$POSTGRES_BIN/pg_ctl" -D "$PGDATA" -m fast stop >/dev/null 2>&1 || true
  fi
  if [[ $status -ne 0 ]]; then
    echo "tmpdir: $TMPDIR" >&2
    echo "postgres log: $PGLOG" >&2
    echo "server log: $SERVERLOG" >&2
    [[ -f "$PGLOG" ]] && cat "$PGLOG" >&2
    [[ -f "$SERVERLOG" ]] && cat "$SERVERLOG" >&2
    [[ -f "$TMPDIR/sse.log" ]] && cat "$TMPDIR/sse.log" >&2
  else
    rm -rf "$TMPDIR"
  fi
}
trap cleanup EXIT

free_port() {
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

json_get() {
  python3 -c 'import json, sys; print(json.load(sys.stdin)[sys.argv[1]])' "$1"
}

assert_json() {
  python3 -c 'import json, sys
data = json.load(sys.stdin)
expr = sys.argv[1]
if not eval(expr, {"json": json}, {"data": data}):
    raise SystemExit(f"assertion failed: {expr}\n{json.dumps(data, indent=2)}")' "$1"
}

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
  for _ in {1..100}; do
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
    sleep 0.05
  done
  echo "missing SSE resource update for $uri" >&2
  return 1
}

wait_for_sse_method() {
  local method="$1"
  for _ in {1..100}; do
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
    sleep 0.05
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
    -H "Origin: http://localhost:$HTTP_PORT"
  )
  if [[ -n "$session" ]]; then
    extra_headers+=(
      -H "MCP-Session-Id: $session"
      -H "MCP-Protocol-Version: 2025-11-25"
    )
  fi
  curl -fsS -D "$headers" -o "$out" -X POST "${extra_headers[@]}" --data "$body" "http://127.0.0.1:$HTTP_PORT/mcp"
  cat "$out"
}

mcp_init() {
  local version="${1:-2025-11-25}"
  local out="$TMPDIR/init-body.json"
  local headers="$TMPDIR/init-headers.txt"
  curl -fsS -D "$headers" -o "$out" -X POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -H "Origin: http://localhost:$HTTP_PORT" \
    --data "$(python3 -c 'import json, sys; print(json.dumps({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":sys.argv[1],"capabilities":{},"clientInfo":{"name":"mcp-smoke","version":"0"}}}))' "$version")" \
    "http://127.0.0.1:$HTTP_PORT/mcp" >/dev/null
  assert_json 'data["result"]["protocolVersion"] == "'"$version"'" and data["result"]["capabilities"]["resources"]["subscribe"] is True and data["result"]["capabilities"]["resources"]["listChanged"] is True and "tools" in data["result"]["capabilities"]' <"$out"
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

PG_PORT="$(free_port)"
HTTP_PORT="$(free_port)"

find_postgres_bin
"$POSTGRES_BIN/initdb" -D "$PGDATA" -A trust -U postgres >/dev/null
"$POSTGRES_BIN/pg_ctl" -D "$PGDATA" -o "-h 127.0.0.1 -p $PG_PORT -k $TMPDIR" -l "$PGLOG" start >/dev/null
POSTGRES_PID=1
for _ in {1..100}; do
  if "$POSTGRES_BIN/pg_isready" -h 127.0.0.1 -p "$PG_PORT" -U postgres >/dev/null 2>&1; then
    break
  fi
  sleep 0.05
done
"$POSTGRES_BIN/createdb" -h 127.0.0.1 -p "$PG_PORT" -U postgres agent_mail_mcp_test

cargo build --manifest-path "$ROOT/Cargo.toml" -p agent-mail-server >/dev/null
DATABASE_URL="postgres://postgres@127.0.0.1:$PG_PORT/agent_mail_mcp_test"
RUST_LOG=warn "$ROOT/target/debug/agent-mail-server" \
  --database-url "$DATABASE_URL" \
  --bind "127.0.0.1:$HTTP_PORT" \
  --token "$TOKEN" >"$SERVERLOG" 2>&1 &
SERVER_PID=$!

for _ in {1..100}; do
  if curl -fsS "http://127.0.0.1:$HTTP_PORT/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.05
done

unauth_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$unauth_status" == "401" ]]
wrong_token_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST -H "Authorization: Bearer wrong" -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$wrong_token_status" == "401" ]]
bad_origin_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST -H "Authorization: Bearer $TOKEN" -H "Origin: https://evil.example" -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$bad_origin_status" == "403" ]]

receiver_session="$(mcp_init)"
sender_session="$(mcp_init)"
[[ -n "$receiver_session" && -n "$sender_session" && "$receiver_session" != "$sender_session" ]]
legacy_session="$(mcp_init "2025-03-26")"
[[ -n "$legacy_session" ]]

unauth_sse_status="$(curl -m 2 -sS -o /dev/null -w "%{http_code}" -H "Accept: text/event-stream" -H "MCP-Session-Id: $receiver_session" "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$unauth_sse_status" == "401" ]]
wrong_token_sse_status="$(curl -m 2 -sS -o /dev/null -w "%{http_code}" -H "Authorization: Bearer wrong" -H "Accept: text/event-stream" -H "MCP-Session-Id: $receiver_session" "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$wrong_token_sse_status" == "401" ]]
stale_session_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream" -H "MCP-Session-Id: mcp-stale" -H "MCP-Protocol-Version: 2025-11-25" --data '{"jsonrpc":"2.0","id":99,"method":"ping"}' "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$stale_session_status" == "404" ]]
notification_status="$(curl -sS -D "$TMPDIR/notification-headers.txt" -o "$TMPDIR/notification-body.txt" -w "%{http_code}" -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream" -H "MCP-Session-Id: $receiver_session" -H "MCP-Protocol-Version: 2025-11-25" --data '{"jsonrpc":"2.0","method":"ping"}' "http://127.0.0.1:$HTTP_PORT/mcp")"
[[ "$notification_status" == "202" ]]
[[ ! -s "$TMPDIR/notification-body.txt" ]]

mcp_post "$receiver_session" '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null
mcp_post "$sender_session" '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null

receiver_start="$(tool_call "$receiver_session" 2 agent_mail_start '{"role":"reviewer"}')"
receiver_identity="$(printf '%s' "$receiver_start" | python3 -c 'import json, sys; print(json.loads(json.load(sys.stdin)["result"]["content"][0]["text"])["identity"])')"
tool_call "$sender_session" 3 agent_mail_start '{"role":"sender"}' >/dev/null

curl -fsS -N \
  -H "Authorization: Bearer $TOKEN" \
  -H "Accept: text/event-stream" \
  -H "MCP-Session-Id: $receiver_session" \
  -H "MCP-Protocol-Version: 2025-11-25" \
  -H "Origin: http://localhost:$HTTP_PORT" \
  "http://127.0.0.1:$HTTP_PORT/mcp" >"$TMPDIR/sse.log" &
SSE_PID=$!
sleep 0.2

tool_call "$sender_session" 4 agent_mail_project_add '{"alias":"mcp-smoke","root":"/tmp/mcp-smoke"}' >/dev/null
wait_for_sse_method "notifications/resources/list_changed"

inbox_uri="agent-mail://projects/mcp-smoke/inbox?identity=$receiver_identity"
mcp_post "$receiver_session" "$(mcp_request resources/list 40 '{}')" | assert_json 'any(item["uri"] == "agent-mail://projects" for item in data["result"]["resources"]) and any(item["uri"] == "'"$inbox_uri"'" for item in data["result"]["resources"])'
mcp_post "$receiver_session" "$(mcp_request resources/templates/list 41 '{}')" | assert_json 'any(item["name"] == "project-inbox" for item in data["result"]["resourceTemplates"]) and any(item["name"] == "project-message" for item in data["result"]["resourceTemplates"])'
mcp_post "$receiver_session" "$(python3 -c 'import json, sys; print(json.dumps({"jsonrpc":"2.0","id":5,"method":"resources/subscribe","params":{"uri":sys.argv[1]}}))' "$inbox_uri")" | assert_json 'data["result"] == {}'

tool_call "$sender_session" 6 agent_mail_send '{"project":"mcp-smoke","to":"reviewer","subject":"MCP smoke","body":"real mcp body"}' >/dev/null
wait_for_sse_uri "$inbox_uri"

inbox_json="$(mcp_post "$receiver_session" "$(python3 -c 'import json, sys; print(json.dumps({"jsonrpc":"2.0","id":7,"method":"resources/read","params":{"uri":sys.argv[1]}}))' "$inbox_uri")")"
printf '%s' "$inbox_json" | assert_json 'json.loads(data["result"]["contents"][0]["text"])["unread_count"] == 1'
mail_id="$(printf '%s' "$inbox_json" | python3 -c 'import json, sys; data=json.loads(json.load(sys.stdin)["result"]["contents"][0]["text"]); print(data["messages"][0]["id"])')"

message_uri="agent-mail://projects/mcp-smoke/messages/$mail_id?identity=$receiver_identity"
mcp_post "$receiver_session" "$(python3 -c 'import json, sys; print(json.dumps({"jsonrpc":"2.0","id":42,"method":"resources/subscribe","params":{"uri":sys.argv[1]}}))' "$message_uri")" | assert_json 'data["result"] == {}'
mcp_post "$receiver_session" "$(python3 -c 'import json, sys; print(json.dumps({"jsonrpc":"2.0","id":8,"method":"resources/read","params":{"uri":sys.argv[1]}}))' "$message_uri")" | assert_json 'json.loads(data["result"]["contents"][0]["text"])["body"] == "real mcp body"'

tool_call "$receiver_session" 9 agent_mail_mark_read "$(python3 -c 'import json, sys; print(json.dumps({"project":"mcp-smoke","mail_id":sys.argv[1]}))' "$mail_id")" >/dev/null
wait_for_sse_uri "$message_uri"
mcp_post "$receiver_session" "$(python3 -c 'import json, sys; print(json.dumps({"jsonrpc":"2.0","id":10,"method":"resources/read","params":{"uri":sys.argv[1]}}))' "$inbox_uri")" | assert_json 'json.loads(data["result"]["contents"][0]["text"])["unread_count"] == 0'

echo "real postgres/mcp test passed"
