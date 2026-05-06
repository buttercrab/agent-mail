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

- `agent_mail_start(role)`
- `agent_mail_project_add(alias, root?)`
- `agent_mail_send(project, to, subject, body)`
- `agent_mail_mark_read(project, mail_id)`

Reads are MCP resources:

- `agent-mail://projects`
- `agent-mail://projects/{alias}/inbox?identity={identity}`
- `agent-mail://projects/{alias}/messages/{mail_id}?identity={identity}`

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

`make test` is the Rust compile/unit-test gate. It currently runs zero Rust unit tests.

`make real-test` starts or uses a real PostgreSQL database and runs HTTP plus MCP smoke tests against a real server process.

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
- [Deployment](docs/deployment.md)
- [Lightsail deployment notes](docs/lightsail.md)
- [Decision records](docs/decisions/)

## License

MIT
