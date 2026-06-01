# Changelog

All notable changes to Agent Mail will be documented here.

This project follows semantic versioning after the first tagged release.

## Unreleased

- Added durable MCP identity: `agent_mail_start` accepts an optional `identity` so an agent can resume the same mailbox across reconnects and restarts.
- Added `agent_mail_drain(project)` tool that returns unread message bodies and marks them read in one transaction.
- Session-scoped tools (`agent_mail_drain`, `agent_mail_send`, `agent_mail_mark_read`) now self-bind: pass `identity` (and `role`) to act in a single call without a separate `agent_mail_start`, so an agent nudged by the unread badge or a hook can read/send/acknowledge immediately.
- Added a compact `agent_mail` unread badge to every tool result (`unread_total` plus per-project counts) so mail surfaces without polling or blocking.
- Bound MCP resource reads/subscriptions to the session identity, so a session can no longer read another participant's inbox or messages via the URI.
- Added `DELETE /mcp` for explicit session teardown and pruning of dead SSE senders.
- Added `agent-mail-notify claude-channel-serve`, a spawned stdio Claude Code channel server (replacing the one-shot `claude-channel-once`), backed by a new streaming `watch_inbox` engine with reconnect and dedup.
- Added fail-open `SessionStart` and `UserPromptSubmit` hook scripts that surface unread mail as session context.
- Fixed a Claude Code channel startup collision: the `claude-channel-serve` stdio server now reports a unique `serverInfo.name` (`agent-mail-channel`) so it loads alongside the `agent-mail` HTTP server without one failing to register (which previously broke `--dangerously-load-development-channels` resolution).
- Extended CI's real-PostgreSQL smoke suite (`scripts/real_postgres_adapter_test.sh`, part of `make real-test`) to exercise the notify adapters (`codex-wait`, `claude-channel-serve`) and the `SessionStart`/`UserPromptSubmit` hook scripts against a throwaway local server, so a server-side change that breaks the client crates or hooks now fails CI instead of slipping through.

## v0.1.0 - 2026-05-06

- Imported Agent Mail service into dedicated repository.
- Added durable setup plan, progress tracker, and decision records.
- Added strict CI with formatting, clippy, Rust tests, and real PostgreSQL HTTP/MCP smoke tests.
- Added staging and production deploy workflows with real public MCP/SSE smoke validation.
- Added separate same-host staging deployment using an isolated PostgreSQL database, systemd service, bind port, install root, token, DNS name, and Cloudflare Origin certificate.
- Added `/health` environment reporting so deploy smoke tests can reject the wrong environment.
- Added OSS project baseline documentation, security policy, contribution guide, Docker packaging files, Dependabot, issue templates, PR template, and CODEOWNERS.
