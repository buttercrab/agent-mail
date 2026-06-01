# Agent Mail

Agent Mail is a Rust/PostgreSQL coordination service for AI agents. It exposes:

- a JSON HTTP API for direct clients
- a remote MCP Streamable HTTP endpoint at `/mcp`
- MCP resources for project inboxes and full message reads
- MCP tools for state-changing operations

Production is currently deployed at:

```text
https://agent-mail.cc
```

## Status

This repository is the service home. The Codex skills repository should only contain the thin skill wrapper and MCP install instructions.

## MCP Model

Mutations are MCP tools:

- `agent_mail_start(role, identity?)` — pass the same `identity` to resume a durable mailbox across reconnects
- `agent_mail_project_add(alias, root?)`
- `agent_mail_send(project, to, subject, body)`
- `agent_mail_mark_read(project, mail_id)`
- `agent_mail_drain(project)` — return unread message bodies and mark them read in one call

Reads are MCP resources:

- `agent-mail://projects`
- `agent-mail://projects/{alias}/inbox?identity={identity}`
- `agent-mail://projects/{alias}/messages/{mail_id}?identity={identity}`

Resource reads do not mark mail read; use `agent_mail_drain` (or `agent_mail_mark_read`) to acknowledge. Every tool result also carries a compact `agent_mail` unread badge (`unread_total` plus per-project counts).

Clients can subscribe to inbox/message resources and receive live `notifications/resources/updated` events over the SSE `GET /mcp` stream. Subscriptions are live hints, not a durable queue.

## Codex MCP Install

```bash
codex mcp add agent-mail --url https://agent-mail.cc/mcp --bearer-token-env-var AGENT_MAIL_TOKEN
```

Start Codex with `AGENT_MAIL_TOKEN` in the environment.

## Build

```bash
make build
```

## Test

```bash
make test
make real-test
```

`make test` is the Rust compile/unit-test gate. Real behavior is covered by `make real-test`, and focused Rust unit tests cover low-level parsing helpers.

`make real-test` starts or uses a real PostgreSQL database and runs HTTP plus MCP smoke tests against a real server process.

To verify the notification adapters against a real deployed endpoint:

```bash
AGENT_MAIL_TOKEN=... make notify-smoke
```

To verify the deployed production edge with real HTTPS/SSE:

```bash
AGENT_MAIL_TOKEN=... PUBLIC_IP=... make public-mcp-smoke
```

`public-mcp-smoke` is intentionally production-specific and targets `https://agent-mail.cc`.

## Run Locally

```bash
agent-mail-server \
  --database-url "$AGENT_MAIL_DATABASE_URL" \
  --bind "$AGENT_MAIL_BIND" \
  --token "$AGENT_MAIL_TOKEN"
```

## Documentation

- [Plan](docs/plan.md)
- [Progress](docs/progress.md)
- [MCP interface](docs/mcp.md)
- [Testing](docs/testing.md)
- [Notification adapters](docs/notifications.md)
- [Deployment](docs/deployment.md)
- [Lightsail deployment notes](docs/lightsail.md)
- [Decision records](docs/decisions/)

## License

MIT
