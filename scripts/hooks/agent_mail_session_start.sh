#!/bin/sh
# Agent Mail SessionStart hook.
#
# Fail-OPEN: this hook only loads context. It must never block a session, so it
# always exits 0 even on network/jq/auth failure. Plain stdout on exit 0 is
# folded into Claude Code's session context.
#
# It GETs the unread inbox over HTTP and prints a compact summary. It does NOT
# mark mail read (that is agent_mail_drain's job) and never prints the token.
#
# Required env (export in your shell or settings.json env block):
#   AGENT_MAIL_URL       default https://agent-mail.cc
#   AGENT_MAIL_TOKEN     bearer token (secret; keep out of settings.json)
#   AGENT_MAIL_PROJECT   project alias
#   AGENT_MAIL_IDENTITY  participant identity

set -u

URL="${AGENT_MAIL_URL:-https://agent-mail.cc}"
TOKEN="${AGENT_MAIL_TOKEN:-}"
PROJECT="${AGENT_MAIL_PROJECT:-}"
IDENTITY="${AGENT_MAIL_IDENTITY:-}"

if [ -z "$TOKEN" ] || [ -z "$PROJECT" ] || [ -z "$IDENTITY" ]; then
  echo "Agent Mail not configured. To enable unread-mail context, export:"
  echo "  export AGENT_MAIL_IDENTITY=worker-001"
  echo "  export AGENT_MAIL_PROJECT=my-project"
  echo "  export AGENT_MAIL_URL=https://agent-mail.cc"
  echo "  export AGENT_MAIL_TOKEN=...   # secret, keep out of settings.json"
  exit 0
fi

body="$(curl -fsS \
  -H "Authorization: Bearer $TOKEN" \
  "$URL/v1/projects/$PROJECT/participants/$IDENTITY/inbox" 2>/dev/null)"

if [ -z "$body" ]; then
  echo "Agent Mail: could not reach inbox for $PROJECT (network/auth); call agent_mail_drain to check."
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
  else
    echo "Agent Mail: inbox clear in $PROJECT"
  fi
else
  # Degrade gracefully without jq: best-effort detection, never mark read.
  if printf '%s' "$body" | grep -q '"unread_count":[1-9]'; then
    echo "Agent Mail: unread mail may be present in $PROJECT; call agent_mail_drain to read."
  else
    echo "Agent Mail: inbox clear in $PROJECT (install jq for a detailed summary)."
  fi
fi

exit 0
