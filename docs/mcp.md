# MCP Interface

Agent Mail exposes MCP over Streamable HTTP at:

```text
POST /mcp
GET /mcp
DELETE /mcp
```

`DELETE /mcp` terminates the MCP session identified by the `Mcp-Session-Id` header.

All MCP requests require:

```text
Authorization: Bearer $AGENT_MAIL_TOKEN
```

## Tools

Tools mutate state or establish session identity:

- `agent_mail_start(role, identity?)` — pass the same `identity` to resume a durable mailbox across reconnects
- `agent_mail_project_add(alias, root?)`
- `agent_mail_send(project, to, subject, body)`
- `agent_mail_mark_read(project, mail_id)`
- `agent_mail_drain(project, identity?, role?)` — return unread message bodies and mark them read in one call; if the session has not called `agent_mail_start`, pass `identity` (and `role`) to bind the session and read in a single call

Every tool result carries a compact `agent_mail` unread badge (`unread_total` plus per-project counts) so agents see pending mail without polling.

## Resources

Resources read state:

- `agent-mail://projects`
- `agent-mail://projects/{alias}/inbox?identity={identity}`
- `agent-mail://projects/{alias}/messages/{mail_id}?identity={identity}`

Resource reads do not mark mail read; use `agent_mail_drain` (or `agent_mail_mark_read`) to acknowledge.

## Subscriptions

Clients may subscribe to inbox and message resources. Updates are sent as live JSON-RPC notifications over the `GET /mcp` SSE stream:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/resources/updated",
  "params": {
    "uri": "agent-mail://projects/example/inbox?identity=agent-1"
  }
}
```

Subscriptions are in-memory session hints. They are not a durable queue.
