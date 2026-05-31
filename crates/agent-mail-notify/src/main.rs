use std::{env, sync::Arc, time::Duration};

use agent_mail_client::{
    AgentMailClient, AgentMailEvent, WatchTarget, claude_channel_notification,
};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdout},
    sync::{Mutex, mpsc},
};

const SERVER_NAME: &str = "agent-mail";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL: &str = "2025-11-25";

#[derive(Debug, Parser)]
#[command(name = "agent-mail-notify")]
#[command(about = "Agent Mail notification adapters for Codex and Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Wait for one real Agent Mail inbox update and print the typed event as JSON.
    CodexWait(WatchArgs),
    /// Run a long-lived Claude Code stdio channel MCP server that pushes Agent Mail summaries.
    ClaudeChannelServe(WatchArgs),
}

#[derive(Clone, Debug, Parser)]
struct WatchArgs {
    #[arg(long, env = "AGENT_MAIL_URL", default_value = "https://agent-mail.cc")]
    url: String,
    #[arg(long, default_value = "AGENT_MAIL_TOKEN")]
    token_env: String,
    #[arg(long)]
    project: String,
    #[arg(long)]
    identity: String,
    #[arg(long, default_value_t = 30)]
    timeout_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::CodexWait(args) => {
            let event = wait(args).await?;
            println!("{}", serde_json::to_string_pretty(&event)?);
        }
        Command::ClaudeChannelServe(args) => serve_channel(args).await?,
    }
    Ok(())
}

/// Run a long-lived Claude Code stdio channel MCP server.
///
/// The stdin/stdout JSON-RPC handshake is the lifetime anchor and starts
/// immediately, independent of upstream Agent Mail connectivity: a missing token
/// or a down endpoint only disables the watch task, never the handshake. All
/// outbound writes (RPC responses and channel notifications) are serialized
/// through one `Mutex<Stdout>` so they never interleave. Exits 0 on stdin EOF.
async fn serve_channel(args: WatchArgs) -> Result<()> {
    let target = args.target();
    if target.project.trim().is_empty() {
        bail!("--project must not be empty");
    }
    if target.identity.trim().is_empty() {
        bail!("--identity must not be empty");
    }

    // Single serialized writer for the MCP stdout pipe.
    let stdout = Arc::new(Mutex::new(tokio::io::stdout()));

    // Build the upstream watch lazily: a missing token or a failed client build
    // must NOT prevent the handshake from working.
    match env::var(&args.token_env) {
        Ok(token) => match AgentMailClient::new(args.url.clone(), token) {
            Ok(client) => spawn_watch(client, target.clone(), stdout.clone()),
            Err(error) => {
                eprintln!("agent-mail channel: upstream disabled (client build failed: {error:#})");
            }
        },
        Err(_) => {
            eprintln!(
                "agent-mail channel: upstream disabled ({} unset); handshake-only mode",
                args.token_env
            );
        }
    }

    // Stdin handshake loop: this owns the process lifetime.
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            // Ignore garbage lines rather than crashing the pipe.
            continue;
        };
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        match (method, id) {
            ("initialize", Some(id)) => {
                let proto = request
                    .pointer("/params/protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PROTOCOL);
                write_msg(&stdout, &rpc_result(&id, initialize_result(proto))).await?;
            }
            ("notifications/initialized", _) => { /* no reply */ }
            ("ping", Some(id)) => write_msg(&stdout, &rpc_result(&id, json!({}))).await?,
            ("tools/list", Some(id)) => {
                write_msg(&stdout, &rpc_result(&id, json!({ "tools": [] }))).await?;
            }
            ("resources/list", Some(id)) => {
                write_msg(&stdout, &rpc_result(&id, json!({ "resources": [] }))).await?;
            }
            (_, Some(id)) => {
                write_msg(&stdout, &rpc_error(&id, -32601, "Method not found")).await?;
            }
            (_, None) => { /* unknown notification -> ignore */ }
        }
    }

    // stdin EOF -> Claude Code closed the pipe -> graceful exit.
    Ok(())
}

/// Spawn the streaming watcher plus a forwarder that converts each event into a
/// `notifications/claude/channel` line written through the serialized stdout.
fn spawn_watch(client: AgentMailClient, target: WatchTarget, stdout: Arc<Mutex<Stdout>>) {
    let (tx, mut rx) = mpsc::channel::<AgentMailEvent>(32);

    let watch_client = client;
    let watch_target = target.clone();
    tokio::spawn(async move {
        // watch_inbox loops with its own backoff and only returns when the
        // receiver is dropped; errors are logged to stderr inside it.
        let _ = watch_client.watch_inbox(&watch_target, tx).await;
    });

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let summary = build_summary(&target, &event);
            let notification = claude_channel_notification(&event, summary);
            // Swallow write errors: the pipe may be closing.
            let _ = write_msg(&stdout, &notification).await;
        }
    });
}

/// Serialize a single JSON-RPC message as one line and flush it while holding
/// the stdout lock, so responses and notifications never interleave.
async fn write_msg(stdout: &Mutex<Stdout>, value: &Value) -> Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    let mut guard = stdout.lock().await;
    guard.write_all(&line).await?;
    guard.flush().await?;
    Ok(())
}

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Build the `initialize` result that declares the `claude/channel` capability.
fn initialize_result(protocol_version: &str) -> Value {
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "experimental": { "claude/channel": {} },
            "tools": {},
            "resources": {}
        },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
    })
}

/// Build the summary string carried as the channel content. It reads only the
/// unread count, latest sender, and subject from the inbox resource: the body is
/// never included because the channel is fire-and-forget and a durable re-read
/// via `agent_mail_drain` stays mandatory.
fn build_summary(target: &WatchTarget, event: &AgentMailEvent) -> String {
    let project = target.project.as_str();
    let Some(resource) = event.resource.as_ref() else {
        return format!("Agent Mail inbox updated in {project}; call agent_mail_drain to read");
    };
    let count = resource
        .get("unread_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let Some(latest) = latest_message(resource) else {
        return format!("Agent Mail inbox updated in {project}; call agent_mail_drain to read");
    };
    let sender = latest
        .get("sender_identity")
        .and_then(Value::as_str)
        .or_else(|| latest.get("sender_role").and_then(Value::as_str))
        .unwrap_or("someone");
    let subject = latest
        .get("subject")
        .and_then(Value::as_str)
        .unwrap_or("(no subject)");
    format!(
        "{count} unread in {project}; latest from {sender} re: {subject}; \
         call agent_mail_drain {project} to read"
    )
}

/// Pick the newest unread message defensively (max over `(created_at_ns, id)`),
/// without assuming the resource preserves the store's ASC ordering.
fn latest_message(resource: &Value) -> Option<&Value> {
    resource
        .get("messages")
        .and_then(Value::as_array)?
        .iter()
        .max_by(|a, b| message_key(a).cmp(&message_key(b)))
}

fn message_key(message: &Value) -> (i64, &str) {
    let created_at_ns = message
        .get("created_at_ns")
        .and_then(Value::as_i64)
        .unwrap_or(i64::MIN);
    let id = message.get("id").and_then(Value::as_str).unwrap_or("");
    (created_at_ns, id)
}

async fn wait(args: WatchArgs) -> Result<agent_mail_client::AgentMailEvent> {
    let token = env::var(&args.token_env)
        .with_context(|| format!("environment variable {} is required", args.token_env))?;
    let target = args.target();
    validate_watch_args(&target, args.timeout_seconds)?;
    let client = AgentMailClient::new(args.url, token)?;
    client
        .watch_inbox_once(&target, Duration::from_secs(args.timeout_seconds))
        .await
}

fn validate_watch_args(target: &WatchTarget, timeout_seconds: u64) -> Result<()> {
    if target.project.trim().is_empty() {
        bail!("--project must not be empty");
    }
    if target.identity.trim().is_empty() {
        bail!("--identity must not be empty");
    }
    if timeout_seconds == 0 {
        bail!("--timeout-seconds must be greater than zero");
    }
    Ok(())
}

impl WatchArgs {
    fn target(&self) -> WatchTarget {
        WatchTarget {
            project: self.project.clone(),
            identity: self.identity.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SERVER_NAME, build_summary, initialize_result, validate_watch_args};
    use agent_mail_client::{
        AgentMailEvent, AgentMailEventKind, WatchTarget, claude_channel_notification,
    };
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn rejects_zero_timeout() {
        let target = WatchTarget {
            project: "demo".into(),
            identity: "worker".into(),
        };

        assert!(validate_watch_args(&target, 0).is_err());
    }

    #[test]
    fn initialize_result_declares_claude_channel_capability() {
        let result = initialize_result("2025-11-25");

        assert!(
            result
                .pointer("/capabilities/experimental/claude~1channel")
                .is_some(),
            "initialize result must declare capabilities.experimental[\"claude/channel\"]"
        );
        assert_eq!(
            result.pointer("/serverInfo/name").and_then(|v| v.as_str()),
            Some(SERVER_NAME)
        );
        assert_eq!(
            result.pointer("/protocolVersion").and_then(|v| v.as_str()),
            Some("2025-11-25")
        );
    }

    #[test]
    fn channel_notification_carries_drain_summary() {
        let target = WatchTarget {
            project: "demo".into(),
            identity: "worker".into(),
        };
        let event = AgentMailEvent {
            id: "event-1".into(),
            kind: AgentMailEventKind::InboxUpdated,
            resource_uri: Some("agent-mail://projects/demo/inbox?identity=worker".into()),
            resource: Some(json!({
                "project": "demo",
                "unread_count": 1,
                "messages": [
                    { "id": "m1", "sender_identity": "alice", "subject": "hi", "created_at_ns": 5 }
                ]
            })),
            received_at: Utc::now(),
        };

        let summary = build_summary(&target, &event);
        assert!(summary.contains("agent_mail_drain"));
        assert!(summary.contains("alice"));

        let notification = claude_channel_notification(&event, summary);
        assert_eq!(
            notification.get("method").and_then(|m| m.as_str()),
            Some("notifications/claude/channel")
        );
        let content = notification
            .pointer("/params/content")
            .and_then(|c| c.as_str())
            .expect("notification has params.content");
        assert!(!content.is_empty());
        assert!(content.contains("agent_mail_drain"));
    }
}
