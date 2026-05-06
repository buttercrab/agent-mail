# ADR 0002: Real Validation Only

## Status

Accepted

## Context

Agent Mail is coordination infrastructure. A green check that does not exercise real behavior is worse than no check because it creates false confidence.

The important behaviors include:

- PostgreSQL persistence
- bearer-token authentication
- MCP session handling
- MCP resource reads
- SSE resource update notifications
- nginx/Cloudflare behavior for deployed environments

## Decision

Validation must be tied to real evidence.

- Local CI must run the server against a real PostgreSQL instance.
- MCP smoke tests must parse actual JSON-RPC/SSE payloads.
- Staging validation must call the staging public URL.
- Production validation must call the production public URL.
- A command that runs zero tests may be included as a compile gate, but must not be described as behavioral coverage.

## Consequences

- CI is more expensive than pure unit tests.
- Public smoke tests may leave durable records until a cleanup model exists.
- Documentation must distinguish compile checks from behavioral checks.
