# MCP Interface

Agent Mail exposes MCP over Streamable HTTP at:

```text
POST /mcp
GET /mcp
```

All MCP requests require:

```text
Authorization: Bearer $AGENT_MAIL_TOKEN
```

## Tools

Tools mutate state or establish session identity:

- `agent_mail_start(role)`
- `agent_mail_project_add(alias, root?)`
- `agent_mail_send(project, to, subject, body)`
- `agent_mail_mark_read(project, mail_id)`

## Resources

Resources read state:

- `agent-mail://projects`
- `agent-mail://projects/{alias}/inbox?identity={identity}`
- `agent-mail://projects/{alias}/messages/{mail_id}?identity={identity}`

Resource reads do not mark mail read.

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
