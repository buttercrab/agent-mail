#!/bin/sh
# Agent Mail UserPromptSubmit hook.
#
# Fail-OPEN: UserPromptSubmit exit code 2 BLOCKS and ERASES the user's prompt,
# so this hook ALWAYS exits 0, even on network/jq/auth failure. Plain stdout on
# exit 0 is added to context alongside the submitted prompt.
#
# Terse by design (it fires on every prompt): it prints the unread summary line
# only when there is unread mail, and is silent when the inbox is clear or on
# any error. It does NOT mark mail read and never prints the token.
#
# Required env (same as the SessionStart hook):
#   AGENT_MAIL_URL       default https://agent-mail.cc
#   AGENT_MAIL_TOKEN     bearer token (secret)
#   AGENT_MAIL_PROJECT   project alias
#   AGENT_MAIL_IDENTITY  participant identity

set -u

URL="${AGENT_MAIL_URL:-https://agent-mail.cc}"
TOKEN="${AGENT_MAIL_TOKEN:-}"
PROJECT="${AGENT_MAIL_PROJECT:-}"
IDENTITY="${AGENT_MAIL_IDENTITY:-}"

if [ -z "$TOKEN" ] || [ -z "$PROJECT" ] || [ -z "$IDENTITY" ]; then
  exit 0
fi

body="$(curl -fsS \
  -H "Authorization: Bearer $TOKEN" \
  "$URL/v1/projects/$PROJECT/participants/$IDENTITY/inbox" 2>/dev/null)"

if [ -z "$body" ]; then
  exit 0
fi

if command -v jq >/dev/null 2>&1; then
  count="$(printf '%s' "$body" | jq -r '.unread_count // 0' 2>/dev/null)"
  case "$count" in
    ''|*[!0-9]*) count=0 ;;
  esac
  if [ "$count" -gt 0 ]; then
    echo "Agent Mail: $count unread in $PROJECT (call agent_mail_drain to read)"
    printf '%s' "$body" \
      | jq -r '.messages | sort_by(.created_at_ns) | .[-1] | "  latest: \(.sender_identity) re: \(.subject)"' 2>/dev/null \
      || true
  fi
else
  if printf '%s' "$body" | grep -q '"unread_count":[1-9]'; then
    echo "Agent Mail: unread mail may be present in $PROJECT; call agent_mail_drain to read."
  fi
fi

exit 0
