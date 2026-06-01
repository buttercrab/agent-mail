#!/usr/bin/env bash
set -euo pipefail

# Boots a throwaway PostgreSQL + agent-mail-server, then exercises the notify
# adapters (codex-wait, claude-channel-serve) and the GA hook scripts against it.
#
# This guards the client/adapter + hook surfaces that unit tests and the
# HTTP/MCP smokes do NOT cover. A server-side authz change once broke the
# watcher (it subscribed an inbox without agent_mail_start) and slipped past CI
# because this path was untested; codex-wait drives exactly that path here.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPDIR="$(mktemp -d /tmp/agent-mail-adapter-test-XXXXXX)"
PGDATA="$TMPDIR/pgdata"
PGLOG="$TMPDIR/postgres.log"
SERVERLOG="$TMPDIR/server.log"
TOKEN="adapter-test-token"
POSTGRES_BIN="${POSTGRES_BIN:-}"

if ! command -v jq >/dev/null 2>&1; then
  echo "this smoke requires jq (the hooks emit a detailed summary via jq)" >&2
  exit 1
fi

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
    echo "tmpdir: $TMPDIR" >&2
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
"$POSTGRES_BIN/createdb" -h 127.0.0.1 -p "$PG_PORT" -U postgres agent_mail_adapter_test

cargo build --manifest-path "$ROOT/Cargo.toml" >/dev/null
DATABASE_URL="postgres://postgres@127.0.0.1:$PG_PORT/agent_mail_adapter_test"
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

export AGENT_MAIL_URL="http://127.0.0.1:$HTTP_PORT"
export AGENT_MAIL_TOKEN="$TOKEN"

# ---------------------------------------------------------------------------
# 1. Notify adapters: codex-wait (watch_inbox_once -> start + subscribe + read)
#    and the claude-channel-serve handshake, against the local server.
# ---------------------------------------------------------------------------
"$ROOT/scripts/notify_adapter_smoke.sh"

# ---------------------------------------------------------------------------
# 2. GA hooks: SessionStart + UserPromptSubmit shell scripts hit /v1 inbox and
#    fold an unread summary into context. They must surface unread mail without
#    marking it read.
# ---------------------------------------------------------------------------
api_post() {
  curl -fsS -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
    --data "$2" "$AGENT_MAIL_URL$1" >/dev/null
}

HOOK_PROJECT="hook-smoke-$$"
HOOK_IDENTITY="hook-worker-$$"
HOOK_SENDER="hook-sender-$$"
api_post /v1/participants/start "{\"identity\":\"$HOOK_IDENTITY\",\"role\":\"worker\"}"
api_post /v1/participants/start "{\"identity\":\"$HOOK_SENDER\",\"role\":\"planner\"}"
api_post /v1/projects "{\"alias\":\"$HOOK_PROJECT\"}"

export AGENT_MAIL_PROJECT="$HOOK_PROJECT"
export AGENT_MAIL_IDENTITY="$HOOK_IDENTITY"

assert_contains() { # haystack needle label
  case "$1" in
    *"$2"*) : ;;
    *) echo "hook assertion failed ($3): expected to contain '$2' but got: $1" >&2; exit 1 ;;
  esac
}

# Clear inbox: SessionStart announces clear; UserPromptSubmit is silent.
out="$("$ROOT/scripts/hooks/agent_mail_session_start.sh")"
assert_contains "$out" "inbox clear in $HOOK_PROJECT" "SessionStart/clear"
out="$("$ROOT/scripts/hooks/agent_mail_user_prompt_submit.sh")"
[[ -z "$out" ]] || { echo "UserPromptSubmit must be silent on a clear inbox, got: $out" >&2; exit 1; }

# Deliver mail, then both hooks surface "1 unread" and name the sender.
api_post /v1/messages "{\"sender_identity\":\"$HOOK_SENDER\",\"project\":\"$HOOK_PROJECT\",\"to_kind\":\"identity\",\"to\":\"$HOOK_IDENTITY\",\"subject\":\"Hook smoke\",\"body\":\"hook body\"}"

out="$("$ROOT/scripts/hooks/agent_mail_session_start.sh")"
assert_contains "$out" "1 unread in $HOOK_PROJECT" "SessionStart/unread"
assert_contains "$out" "$HOOK_SENDER" "SessionStart/sender"
out="$("$ROOT/scripts/hooks/agent_mail_user_prompt_submit.sh")"
assert_contains "$out" "1 unread in $HOOK_PROJECT" "UserPromptSubmit/unread"

# Hooks must NOT acknowledge mail (only agent_mail_drain / agent_mail_mark_read do).
unread="$(curl -fsS -H "Authorization: Bearer $TOKEN" \
  "$AGENT_MAIL_URL/v1/projects/$HOOK_PROJECT/participants/$HOOK_IDENTITY/inbox" \
  | jq '.unread_count')"
[[ "$unread" == "1" ]] || { echo "hooks must not mark mail read; unread_count=$unread" >&2; exit 1; }

echo "real postgres/adapter+hook test passed"
