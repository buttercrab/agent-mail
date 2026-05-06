# ADR 0003: Staging Before Production

## Status

Accepted

## Context

Agent Mail depends on behavior that local tests cannot fully prove:

- Cloudflare and nginx proxy behavior
- long-lived SSE streams
- TLS/origin behavior
- systemd environment configuration
- managed PostgreSQL connectivity

Running broad smoke tests directly against production creates persistent test records and risks validating changes against live coordination data.

## Decision

Use a real staging environment before production.

Staging must have its own:

- public URL
- bearer token
- PostgreSQL database
- GitHub environment secrets
- deployed systemd/nginx path

Merges to `main` may deploy to staging and run full deployed MCP smoke. Production deploys remain manual and should run a smaller deployed smoke against `https://agent-mail.cc`.

## Consequences

- Staging setup is required before the CD story is complete.
- The server must support environment-specific allowed Origins.
- Production is protected from being the primary validation target.
