#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPDIR="$(mktemp -d /tmp/agent-mail-real-test-XXXXXX)"
PGDATA="$TMPDIR/pgdata"
PGLOG="$TMPDIR/postgres.log"
SERVERLOG="$TMPDIR/server.log"
TOKEN="real-test-token"
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
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  if [[ -n "${POSTGRES_PID:-}" ]]; then
    "$POSTGRES_BIN/pg_ctl" -D "$PGDATA" -m fast stop >/dev/null 2>&1 || true
  fi
  if [[ $status -ne 0 ]]; then
    echo "postgres log: $PGLOG" >&2
    echo "server log: $SERVERLOG" >&2
    [[ -f "$PGLOG" ]] && cat "$PGLOG" >&2
    [[ -f "$SERVERLOG" ]] && cat "$SERVERLOG" >&2
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
if not eval(expr, {}, {"data": data}):
    raise SystemExit(f"assertion failed: {expr}\n{json.dumps(data, indent=2)}")' "$1"
}

request() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -fsS \
      -H "Authorization: Bearer $TOKEN" \
      -H "Content-Type: application/json" \
      -X "$method" \
      --data "$body" \
      "http://127.0.0.1:$HTTP_PORT$path"
  else
    curl -fsS \
      -H "Authorization: Bearer $TOKEN" \
      -X "$method" \
      "http://127.0.0.1:$HTTP_PORT$path"
  fi
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
"$POSTGRES_BIN/createdb" -h 127.0.0.1 -p "$PG_PORT" -U postgres agent_mail_real_test

cargo build --manifest-path "$ROOT/Cargo.toml" >/dev/null
DATABASE_URL="postgres://postgres@127.0.0.1:$PG_PORT/agent_mail_real_test"
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

unauth_status="$(
  curl -sS -o /dev/null -w "%{http_code}" "http://127.0.0.1:$HTTP_PORT/v1/projects"
)"
[[ "$unauth_status" == "401" ]]

request POST /v1/participants/start '{"identity":"sender-001","role":"sender"}' | assert_json 'data["identity"] == "sender-001" and data["role"] == "sender"'
request POST /v1/participants/start '{"identity":"reviewer-001","role":"reviewer"}' | assert_json 'data["identity"] == "reviewer-001" and data["role"] == "reviewer"'
request POST /v1/projects '{"alias":"alpha","root":"/srv/alpha"}' | assert_json 'data["alias"] == "alpha"'
request POST /v1/projects '{"alias":"beta","root":"/srv/beta"}' | assert_json 'data["alias"] == "beta"'

alpha_message="$(
  request POST /v1/messages '{"sender_identity":"sender-001","project":"alpha","to_kind":"role","to":"reviewer","subject":"Alpha review","body":"alpha body"}'
)"
alpha_id="$(printf '%s' "$alpha_message" | json_get id)"
request POST /v1/messages '{"sender_identity":"sender-001","project":"beta","to_kind":"role","to":"reviewer","subject":"Beta review","body":"beta body"}' >/dev/null

request GET /v1/projects/alpha/participants/reviewer-001/inbox | assert_json 'data["unread_count"] == 1 and data["messages"][0]["project"] == "alpha"'
request GET /v1/projects/beta/participants/reviewer-001/inbox | assert_json 'data["unread_count"] == 1 and data["messages"][0]["project"] == "beta"'
request GET "/v1/projects/alpha/messages/$alpha_id?identity=reviewer-001" | assert_json 'data["body"] == "alpha body"'
request POST "/v1/projects/alpha/messages/$alpha_id/read" '{"identity":"reviewer-001"}' | assert_json 'data["marked_read"]'
request GET /v1/projects/alpha/participants/reviewer-001/inbox | assert_json 'data["unread_count"] == 0'
request GET /v1/projects/beta/participants/reviewer-001/inbox | assert_json 'data["unread_count"] == 1'

echo "real postgres/http test passed"
