# Changelog

All notable changes to Agent Mail will be documented here.

This project follows semantic versioning after the first tagged release.

## Unreleased

- Added durable MCP identity: `agent_mail_start` accepts an optional `identity` so an agent can resume the same mailbox across reconnects and restarts.
- Added `agent_mail_drain(project)` tool that returns unread message bodies and marks them read in one transaction.
- Added a compact `agent_mail` unread badge to every tool result (`unread_total` plus per-project counts) so mail surfaces without polling or blocking.
- Bound MCP resource reads/subscriptions to the session identity, so a session can no longer read another participant's inbox or messages via the URI.
- Added `DELETE /mcp` for explicit session teardown and pruning of dead SSE senders.
- Added `agent-mail-notify claude-channel-serve`, a spawned stdio Claude Code channel server (replacing the one-shot `claude-channel-once`), backed by a new streaming `watch_inbox` engine with reconnect and dedup.
- Added fail-open `SessionStart` and `UserPromptSubmit` hook scripts that surface unread mail as session context.

## v0.1.0 - 2026-05-06

- Imported Agent Mail service into dedicated repository.
- Added durable setup plan, progress tracker, and decision records.
- Added strict CI with formatting, clippy, Rust tests, and real PostgreSQL HTTP/MCP smoke tests.
- Added staging and production deploy workflows with real public MCP/SSE smoke validation.
- Added separate same-host staging deployment using an isolated PostgreSQL database, systemd service, bind port, install root, token, DNS name, and Cloudflare Origin certificate.
- Added `/health` environment reporting so deploy smoke tests can reject the wrong environment.
- Added OSS project baseline documentation, security policy, contribution guide, Docker packaging files, Dependabot, issue templates, PR template, and CODEOWNERS.
