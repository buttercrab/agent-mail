# Agent Mail Repository Setup Plan

This repository is the production home for Agent Mail: a Rust/PostgreSQL service that exposes a JSON HTTP API and a remote MCP Streamable HTTP endpoint.

## Success Criteria

- The repository builds from a clean checkout.
- CI runs real checks: formatting, clippy with warnings denied, Rust tests, and real PostgreSQL HTTP/MCP smoke tests.
- Deployment validation is real, not assumed:
  - staging uses its own URL, token, and database
  - production deploys are explicit and include post-deploy smoke checks
- The public MCP model is documented:
  - tools mutate state
  - resources read inboxes/messages
  - subscriptions deliver live `notifications/resources/updated`
- OSS hygiene files exist and are accurate:
  - `README.md`
  - `LICENSE`
  - `SECURITY.md`
  - `CONTRIBUTING.md`
  - `CHANGELOG.md`
  - issue and PR templates
- Progress is tracked in `docs/progress.md` with command evidence.

## Non-Goals For The Initial Import

- No fake unit tests.
- No placeholder CI that reports success without running real service checks.
- No production auto-deploy on every merge.
- No cleanup/delete API solely to make smoke tests tidy.
- No major crate split until the imported service is green in the new repository.

## Phases

1. Create durable planning and progress docs.
2. Import the existing Agent Mail service from the skills repository.
3. Add strict Rust and OSS baseline configuration.
4. Add local real validation scripts and CI workflows.
5. Add staging deployment validation.
6. Add production release/deploy workflow with manual approval.
7. Audit the repository against the success criteria.

## Architecture Direction

Keep the first import conservative. The current server can remain one crate while the new repository establishes reliable CI/CD and docs. Future modularization should follow this dependency direction:

```text
server -> http/mcp -> core + store
store  -> core
http   -> core
mcp    -> core
core   -> no infrastructure dependencies
```

Do not split crates until the benefit is concrete and the tests remain green after each step.
