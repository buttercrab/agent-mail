# Agent Mail Notification Adapters

Agent Mail's server-side MCP behavior stays protocol-focused:

- tools mutate mail state
- resources read mail state
- `resources/subscribe` registers live resource interests
- `GET /mcp` emits `notifications/resources/updated` over SSE

Client notification UX belongs in adapters. Codex and Claude Code expose different runtime surfaces, so this repository keeps one shared watcher implementation and thin client-specific binaries.

## Shared Watcher

The shared watcher connects to a real Agent Mail MCP endpoint, initializes a session, subscribes to one or more resource URIs, opens the MCP SSE stream, and emits typed events when matching JSON-RPC notifications arrive.

SSE notifications are invalidation hints, not durable delivery. A correct watcher re-reads the subscribed resource after matching updates and also polls while waiting so mail that arrived during startup or a transient stream gap is still observed from PostgreSQL-backed state.

It must handle:

- bearer-token authentication
- MCP session IDs
- protocol-version headers
- SSE parsing
- timeout handling
- reconnect behavior before long-running use is considered production-ready

The first supported resource is the project inbox:

```text
agent-mail://projects/{alias}/inbox?identity={identity}
```

## Codex Adapter

Current Codex sessions can call MCP tools and read MCP resources, but this runtime does not expose a native callback channel that interrupts the agent when a subscribed MCP resource changes.

The Codex adapter therefore provides an explicit wait/poll surface:

```bash
cargo run -p agent-mail-notify -- codex-wait \
  --project my-project \
  --identity worker-001 \
  --role worker \
  --timeout-seconds 30
```

The command blocks until Agent Mail emits a matching inbox update, then prints a JSON event. This is not transparent background push, but it is real, observable, and compatible with Codex today.

## Claude Code Adapter

Claude Code has a client-specific channel extension. The Claude adapter bridges Agent Mail MCP resource updates into Claude channel notifications while keeping the public Agent Mail server spec-clean. The channel schema follows the [Claude Code Channels reference](https://code.claude.com/docs/en/channels-reference): `notifications/claude/channel` with `content` and optional `meta`.

There are two delivery surfaces:

1. **Channels** (research preview, version/org gated) — true between-turn push.
2. **Hooks** (GA, un-gated, works today) — reliable fallback that injects an unread summary into context on session start and on every prompt.

### Channel server (`claude-channel-serve`)

A Claude Code "channel" is a long-running stdio MCP server that Claude Code **spawns as a subprocess**. The server declares `capabilities.experimental["claude/channel"] = {}` in its `initialize` result and, when mail arrives, emits a JSON-RPC notification on its **own stdout** between turns:

```text
notifications/claude/channel
```

Claude Code wraps that notification's `content` into a `<channel source="agent-mail">…</channel>` tag the model reads on the next turn. The `source` attribute is set automatically from the configured server name, not from `meta`.

Launch the server with:

```bash
agent-mail-notify claude-channel-serve --project my-project --identity worker-001 --role worker
```

It reads the inbox over the shared streaming watcher (auto-reconnect with backoff, durable identity, high-water-mark dedup so each unread batch is announced at most once) and never returns on a single event.

Register it so Claude Code spawns it (`.mcp.json` / `~/.claude.json` `mcpServers`):

```json
{
  "mcpServers": {
    "agent-mail": {
      "command": "agent-mail-notify",
      "args": ["claude-channel-serve", "--project", "my-project", "--identity", "worker-001", "--role", "worker"],
      "env": {
        "AGENT_MAIL_URL": "https://agent-mail.cc",
        "AGENT_MAIL_TOKEN": "..."
      }
    }
  }
}
```

The handshake (`initialize`/`ping`/`tools/list`/`resources/list`) responds immediately even if the upstream Agent Mail endpoint is unreachable (missing token or network down only disables the watch task), and the server exits cleanly (0) when Claude Code closes the stdin pipe.

### Launch flag and gating

Channels are a **research preview** and are **version/org gated**. The channel listener is only loaded when Claude Code is started with the development-channels flag:

```bash
claude --dangerously-load-development-channels server:agent-mail
```

That dev flag bypasses the Anthropic allowlist per entry (it prompts for confirmation). Once allowlisting is GA, an allowlisted entry is enabled via `--channels` instead. Additional gating to be aware of:

- Requires Claude Code **v2.1.80+**.
- Requires Anthropic auth (claude.ai login **or** a Console API key). Channels are **not** available on Amazon Bedrock, Google Vertex AI, or Microsoft Foundry.
- Org policy: claude.ai Team/Enterprise blocks channels until an admin sets the managed `channelsEnabled` setting. The dev flag bypasses the allowlist but **not** `channelsEnabled`.

When channels are not enabled the MCP server still connects and its tools work, but channel notifications never arrive.

### Fire-and-forget and durable re-read

Channel delivery is **fire-and-forget**: writing the notification line resolves when it is written to the transport, not when Claude processed it. If the session did not load the server as a channel, or org policy blocks it, the event is **dropped silently** with no error to the server. Events are delivered **between turns**, grouped, never as a mid-turn interrupt, and only while the session is open.

Because delivery can be silently dropped, the channel `content` is only a **summary that nudges `agent_mail_drain`**, never the message body:

```text
2 unread in my-project; latest from alice re: deploy plan; call agent_mail_drain my-project to read
```

Durable re-read via `agent_mail_drain` (or the inbox resource) stays mandatory.

### GA hook fallback (un-gated, works today)

Two POSIX hooks are the reliable fallback that works regardless of channel gating. They `GET` the unread inbox over HTTP and print a compact summary into context. They are **fail-open** (always exit 0, never block the session, never print the token) and **never mark mail read** — that remains `agent_mail_drain`'s job.

- `scripts/hooks/agent_mail_session_start.sh` — runs on SessionStart, prints the unread summary plus the env-export documentation.
- `scripts/hooks/agent_mail_user_prompt_submit.sh` — runs on every UserPromptSubmit, terse: prints the unread line only when there is unread mail, silent otherwise.

Wire both in `settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [ { "type": "command", "command": "$CLAUDE_PROJECT_DIR/scripts/hooks/agent_mail_session_start.sh" } ] }
    ],
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": "$CLAUDE_PROJECT_DIR/scripts/hooks/agent_mail_user_prompt_submit.sh" } ] }
    ]
  },
  "env": {
    "AGENT_MAIL_URL": "https://agent-mail.cc",
    "AGENT_MAIL_PROJECT": "my-project",
    "AGENT_MAIL_IDENTITY": "worker-001"
  }
}
```

`AGENT_MAIL_TOKEN` is a secret and should come from the shell/secret env, not be committed to `settings.json`. The hooks degrade gracefully without `jq` and stay silent (exit 0) on any network/auth failure.

### Spec-clean note

The bridge is intentionally separate from the server. The Agent Mail Rust server emits no Claude-specific methods; `notifications/claude/channel` exists only in the local `agent-mail-notify` adapter.

## Validation

Notification work is not complete unless it is proven against real endpoints.

Required gates:

- local unit tests for parsing and event classification
- local real PostgreSQL MCP smoke tests
- deployed MCP smoke against `https://agent-mail.cc/mcp`
- adapter smoke where a watcher subscribes, a real message is sent, and the watcher receives the update

Avoid claims based only on health checks or fake in-memory tests.
