#!/usr/bin/env bash
set -euo pipefail

AGENT_MAIL_URL=https://agent-mail.cc exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/deployed_mcp_smoke.sh"
