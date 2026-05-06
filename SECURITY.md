# Security Policy

## Supported Versions

Agent Mail is pre-1.0. Security fixes target the latest `main` branch and the latest tagged release once releases begin.

## Reporting A Vulnerability

Do not open a public issue for vulnerabilities involving authentication, deployment secrets, database exposure, or MCP transport behavior.

Report privately to the repository owner. Include:

- affected version or commit
- deployment mode, if relevant
- reproduction steps
- impact
- whether secrets or production data may be involved

## Security Model

- All non-health HTTP and MCP endpoints require `Authorization: Bearer $AGENT_MAIL_TOKEN`.
- `AGENT_MAIL_TOKEN` is required at server startup.
- Port `8787` is intended to be private behind nginx/Cloudflare in production.
- MCP Origin validation is enforced for browser-origin requests.
- Message bodies are readable only for delivered mail identities.
