# ADR 0001: Repository Scope

## Status

Accepted

## Context

Agent Mail started inside the `skills` repository as a Codex skill plus a Rust/PostgreSQL service. It now has deployed infrastructure, an HTTP API, a remote MCP endpoint, integration smoke tests, and operational documentation.

Keeping production service code inside the skills repository mixes two responsibilities:

- local Codex skill distribution
- production service development and operations

## Decision

Create a dedicated `agent-mail` repository for the production service.

The skills repository should become a thin wrapper that points Codex users at the deployed MCP URL and this service repository.

## Consequences

- CI/CD, releases, and deployment history live with the service code.
- The service can use stricter Rust and deployment gates without affecting unrelated skills.
- The initial import should avoid unnecessary refactors so behavior remains verifiable.
