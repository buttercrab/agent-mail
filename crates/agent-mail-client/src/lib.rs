use std::{fmt, time::Duration};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, stream::BoxStream};
use reqwest::{
    Client, Response, StatusCode,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{sync::mpsc, time};

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const MCP_SESSION_ID: &str = "mcp-session-id";
const MCP_PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";

#[derive(Clone, Debug)]
pub struct AgentMailClient {
    http: Client,
    base_url: String,
    token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchTarget {
    pub project: String,
    pub identity: String,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMailEventKind {
    InboxUpdated,
    MessageUpdated,
    ProjectsUpdated,
    ResourceListChanged,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentMailEvent {
    pub id: String,
    pub kind: AgentMailEventKind,
    pub resource_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Value>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct RpcEnvelope {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Error)]
pub enum AgentMailClientError {
    #[error("MCP request failed with HTTP {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("MCP response did not include {MCP_SESSION_ID}")]
    MissingSessionId,
    #[error("MCP returned error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("timed out waiting for Agent Mail event")]
    Timeout,
}

impl AgentMailClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(anyhow!("Agent Mail URL is empty"));
        }
        let token = token.into();
        if token.is_empty() {
            return Err(anyhow!("Agent Mail token is empty"));
        }
        Ok(Self {
            http: Client::builder()
                .user_agent(concat!(
                    "agent-mail-client/",
                    env!("CARGO_PKG_VERSION"),
                    " ",
                    env!("CARGO_PKG_REPOSITORY")
                ))
                .build()
                .context("build HTTP client")?,
            base_url,
            token,
        })
    }

    pub async fn watch_inbox_once(
        &self,
        target: &WatchTarget,
        timeout: Duration,
    ) -> Result<AgentMailEvent> {
        let session_id = self.initialize().await?;
        let outcome: Result<AgentMailEvent> = async {
            self.initialized(&session_id).await?;
            self.start(&session_id, target).await?;

            let inbox_uri = target.inbox_uri();
            self.subscribe(&session_id, &inbox_uri).await?;

            let response = self.open_sse(&session_id).await?;
            let mut sse = SseJsonStream::new(response);
            if let Some(resource) = self.read_resource(&session_id, &inbox_uri).await?
                && inbox_has_unread(&resource)
            {
                return Ok(resource_event(
                    AgentMailEventKind::InboxUpdated,
                    Some(inbox_uri),
                    Some(resource),
                ));
            }

            let mut poll_interval = time::interval(Duration::from_secs(1));
            poll_interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
            let event = time::timeout(timeout, async {
            loop {
                tokio::select! {
                    payload = sse.next_json() => {
                        let payload = payload?
                            .ok_or_else(|| anyhow!("MCP SSE stream ended before event arrived"))?;
                        if let Some(event) = classify_notification(&payload)
                            && event.resource_uri.as_deref() == Some(inbox_uri.as_str())
                        {
                            let resource = self.read_resource(&session_id, &inbox_uri).await?;
                            if resource.as_ref().is_some_and(inbox_has_unread) {
                                return Ok::<AgentMailEvent, anyhow::Error>(AgentMailEvent {
                                    resource,
                                    ..event
                                });
                            }
                        }
                    }
                    _ = poll_interval.tick() => {
                        if let Some(resource) = self.read_resource(&session_id, &inbox_uri).await?
                            && inbox_has_unread(&resource)
                        {
                            return Ok::<AgentMailEvent, anyhow::Error>(resource_event(
                                AgentMailEventKind::InboxUpdated,
                                Some(inbox_uri.clone()),
                                Some(resource),
                            ));
                        }
                    }
                }
            }
        })
        .await
        .map_err(|_| AgentMailClientError::Timeout)??;

            Ok(event)
        }
        .await;
        let _ = self.delete_session(&session_id).await;
        outcome
    }

    /// Stream newly-observed inbox events to `tx` until the receiver is dropped.
    ///
    /// Unlike [`Self::watch_inbox_once`], this loops forever: it rebuilds a fresh
    /// remote MCP session on every reconnect (durable identity is re-derived from
    /// the stable `target` each time), applies exponential backoff capped near 30s
    /// on stream errors, and de-duplicates against a high-water mark that persists
    /// across reconnects so a given unread message is emitted at most once.
    pub async fn watch_inbox(
        &self,
        target: &WatchTarget,
        tx: mpsc::Sender<AgentMailEvent>,
    ) -> Result<()> {
        const BACKOFF_BASE: Duration = Duration::from_millis(500);
        const BACKOFF_CAP: Duration = Duration::from_secs(30);

        let mut backoff = BACKOFF_BASE;
        // (max created_at_ns, id) high-water mark; persists across reconnects.
        let mut hwm: Option<(i64, String)> = None;
        loop {
            match self
                .watch_session(target, &tx, &mut hwm, &mut backoff)
                .await
            {
                // Ok(()) only when the receiver was dropped -> graceful stop.
                Ok(()) => return Ok(()),
                Err(error) => {
                    eprintln!("agent-mail watch session error: {error:#}");
                }
            }
            tokio::select! {
                _ = tx.closed() => return Ok(()),
                _ = time::sleep(backoff) => {}
            }
            backoff = (backoff * 2).min(BACKOFF_CAP);
        }
    }

    /// Run one MCP session for [`Self::watch_inbox`]: connect, prime, then loop
    /// over SSE notifications and a 1s poll. Returns `Ok(())` only when the
    /// receiver has been dropped; any stream/transport failure is an `Err` so the
    /// outer loop reconnects with backoff.
    async fn watch_session(
        &self,
        target: &WatchTarget,
        tx: &mpsc::Sender<AgentMailEvent>,
        hwm: &mut Option<(i64, String)>,
        backoff: &mut Duration,
    ) -> Result<()> {
        let session_id = self.initialize().await?;
        let outcome: Result<()> = async {
        self.initialized(&session_id).await?;
        self.start(&session_id, target).await?;

        let inbox_uri = target.inbox_uri();
        self.subscribe(&session_id, &inbox_uri).await?;

        let response = self.open_sse(&session_id).await?;
        let mut sse = SseJsonStream::new(response);

        // Session established -> reset backoff so a long-lived connection that
        // drops after hours reconnects fast, while a hard-down endpoint stays
        // backed off.
        *backoff = Duration::from_millis(500);

        // Priming read: emit any already-unread mail beyond the high-water mark.
        if let Some(resource) = self.read_resource(&session_id, &inbox_uri).await?
            && !self.emit_if_new(&resource, &inbox_uri, hwm, tx).await?
        {
            return Ok(());
        }

        let mut poll_interval = time::interval(Duration::from_secs(1));
        poll_interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                payload = sse.next_json() => {
                    let payload = payload?
                        .ok_or_else(|| anyhow!("MCP SSE stream ended"))?;
                    if let Some(event) = classify_notification(&payload)
                        && event.resource_uri.as_deref() == Some(inbox_uri.as_str())
                        && let Some(resource) = self.read_resource(&session_id, &inbox_uri).await?
                        && !self.emit_if_new(&resource, &inbox_uri, hwm, tx).await?
                    {
                        return Ok(());
                    }
                }
                _ = poll_interval.tick() => {
                    if let Some(resource) = self.read_resource(&session_id, &inbox_uri).await?
                        && !self.emit_if_new(&resource, &inbox_uri, hwm, tx).await?
                    {
                        return Ok(());
                    }
                }
                _ = tx.closed() => return Ok(()),
            }
        }
        }
        .await;
        let _ = self.delete_session(&session_id).await;
        outcome
    }

    /// Emit an inbox event when the resource carries unread mail strictly newer
    /// than the high-water mark. Advances the mark only after a successful send.
    ///
    /// Returns `Ok(false)` when the receiver has been dropped (caller should
    /// stop), `Ok(true)` otherwise.
    async fn emit_if_new(
        &self,
        resource: &Value,
        inbox_uri: &str,
        hwm: &mut Option<(i64, String)>,
        tx: &mpsc::Sender<AgentMailEvent>,
    ) -> Result<bool> {
        if !inbox_has_unread(resource) {
            return Ok(true);
        }
        let Some(newest) = latest_message_key(resource) else {
            return Ok(true);
        };
        let is_new = hwm.as_ref().is_none_or(|mark| newest > *mark);
        if !is_new {
            return Ok(true);
        }
        let event = resource_event(
            AgentMailEventKind::InboxUpdated,
            Some(inbox_uri.to_string()),
            Some(resource.clone()),
        );
        if tx.send(event).await.is_err() {
            return Ok(false);
        }
        // Only advance the mark on a successful emit so a dropped send retries.
        *hwm = Some(newest);
        Ok(true)
    }

    pub async fn read_resource(&self, session_id: &str, uri: &str) -> Result<Option<Value>> {
        let response = self
            .http
            .post(self.mcp_url())
            .headers(self.session_headers(session_id)?)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "resources/read",
                "params": { "uri": uri }
            }))
            .send()
            .await
            .with_context(|| format!("read MCP resource {uri}"))?;
        let value = read_rpc_result(response).await?;
        extract_resource_text_json(&value)
    }

    async fn initialize(&self) -> Result<String> {
        let response = self
            .http
            .post(self.mcp_url())
            .headers(self.base_headers()?)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "agent-mail-client",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }))
            .send()
            .await
            .context("send MCP initialize")?;
        let session_id = header_string(response.headers(), MCP_SESSION_ID)
            .ok_or(AgentMailClientError::MissingSessionId)?;
        read_rpc_result(response).await?;
        Ok(session_id)
    }

    async fn initialized(&self, session_id: &str) -> Result<()> {
        let response = self
            .http
            .post(self.mcp_url())
            .headers(self.session_headers(session_id)?)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }))
            .send()
            .await
            .context("send MCP initialized notification")?;
        ensure_status(response).await.map(|_| ())
    }

    async fn start(&self, session_id: &str, target: &WatchTarget) -> Result<()> {
        let response = self
            .http
            .post(self.mcp_url())
            .headers(self.session_headers(session_id)?)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "agent_mail_start",
                    "arguments": {
                        "role": target.role.clone(),
                        "identity": target.identity.clone()
                    }
                }
            }))
            .send()
            .await
            .context("call agent_mail_start")?;
        read_rpc_result(response).await?;
        Ok(())
    }

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .http
            .delete(self.mcp_url())
            .headers(self.session_headers(session_id)?)
            .send()
            .await
            .context("delete MCP session")?;
        ensure_status(response).await.map(|_| ())
    }

    async fn subscribe(&self, session_id: &str, uri: &str) -> Result<()> {
        let response = self
            .http
            .post(self.mcp_url())
            .headers(self.session_headers(session_id)?)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "resources/subscribe",
                "params": { "uri": uri }
            }))
            .send()
            .await
            .with_context(|| format!("subscribe to MCP resource {uri}"))?;
        read_rpc_result(response).await?;
        Ok(())
    }

    async fn open_sse(&self, session_id: &str) -> Result<Response> {
        let mut headers = self.session_headers(session_id)?;
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        let response = self
            .http
            .get(self.mcp_url())
            .headers(headers)
            .send()
            .await
            .context("open MCP SSE stream")?;
        ensure_status(response).await
    }

    fn mcp_url(&self) -> String {
        format!("{}/mcp", self.base_url)
    }

    fn base_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.token))
                .context("build Authorization header")?,
        );
        Ok(headers)
    }

    fn session_headers(&self, session_id: &str) -> Result<HeaderMap> {
        let mut headers = self.base_headers()?;
        headers.insert(
            MCP_SESSION_ID,
            HeaderValue::from_str(session_id).context("build MCP session header")?,
        );
        headers.insert(
            MCP_PROTOCOL_VERSION_HEADER,
            HeaderValue::from_static(MCP_PROTOCOL_VERSION),
        );
        Ok(headers)
    }
}

struct SseJsonStream {
    inner: BoxStream<'static, reqwest::Result<Bytes>>,
    buffer: String,
}

impl SseJsonStream {
    fn new(response: Response) -> Self {
        Self {
            inner: response.bytes_stream().boxed(),
            buffer: String::new(),
        }
    }

    async fn next_json(&mut self) -> Result<Option<Value>> {
        while let Some(chunk) = self.inner.next().await {
            let chunk = chunk.context("read MCP SSE chunk")?;
            self.buffer
                .push_str(std::str::from_utf8(&chunk).context("decode MCP SSE as UTF-8")?);
            while let Some((raw_event, rest)) = split_sse_event(&self.buffer) {
                self.buffer = rest;
                if let Some(value) = parse_sse_event(&raw_event)? {
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }
}

impl WatchTarget {
    pub fn inbox_uri(&self) -> String {
        format!(
            "agent-mail://projects/{}/inbox?identity={}",
            encode_component(&self.project),
            encode_component(&self.identity)
        )
    }
}

pub fn claude_channel_notification(event: &AgentMailEvent, content: impl fmt::Display) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/claude/channel",
        "params": {
            "content": content.to_string(),
            "meta": {
                "event_id": event.id,
                "kind": format!("{:?}", event.kind),
                "resource_uri": event.resource_uri.as_deref().unwrap_or("")
            }
        }
    })
}

async fn read_rpc_result(response: Response) -> Result<Value> {
    let response = ensure_status(response).await?;
    let envelope: RpcEnvelope = response
        .json()
        .await
        .context("decode MCP JSON-RPC response")?;
    if let Some(error) = envelope.error {
        return Err(AgentMailClientError::Rpc {
            code: error.code,
            message: error.message,
        }
        .into());
    }
    Ok(envelope.result.unwrap_or_else(|| json!({})))
}

async fn ensure_status(response: Response) -> Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(AgentMailClientError::HttpStatus { status, body }.into())
}

fn split_sse_event(buffer: &str) -> Option<(String, String)> {
    if let Some(index) = buffer.find("\n\n") {
        return Some((buffer[..index].to_string(), buffer[index + 2..].to_string()));
    }
    if let Some(index) = buffer.find("\r\n\r\n") {
        return Some((buffer[..index].to_string(), buffer[index + 4..].to_string()));
    }
    None
}

fn parse_sse_event(raw_event: &str) -> Result<Option<Value>> {
    let mut data = String::new();
    for line in raw_event.lines() {
        let line = line.trim_end_matches('\r');
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        if !data.is_empty() {
            data.push('\n');
        }
        data.push_str(rest.trim_start());
    }
    if data.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&data)
        .context("decode MCP SSE JSON")
        .map(Some)
}

fn classify_notification(value: &Value) -> Option<AgentMailEvent> {
    let method = value.get("method")?.as_str()?;
    match method {
        "notifications/resources/updated" => {
            let uri = value.get("params")?.get("uri")?.as_str()?.to_string();
            Some(resource_event(classify_resource_uri(&uri), Some(uri), None))
        }
        "notifications/resources/list_changed" => Some(resource_event(
            AgentMailEventKind::ResourceListChanged,
            None,
            None,
        )),
        _ => None,
    }
}

fn classify_resource_uri(uri: &str) -> AgentMailEventKind {
    if uri == "agent-mail://projects" {
        AgentMailEventKind::ProjectsUpdated
    } else if uri.contains("/messages/") {
        AgentMailEventKind::MessageUpdated
    } else {
        AgentMailEventKind::InboxUpdated
    }
}

fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn event_id() -> String {
    format!("event-{}", Utc::now().timestamp_micros())
}

fn resource_event(
    kind: AgentMailEventKind,
    resource_uri: Option<String>,
    resource: Option<Value>,
) -> AgentMailEvent {
    AgentMailEvent {
        id: event_id(),
        kind,
        resource_uri,
        resource,
        received_at: Utc::now(),
    }
}

fn extract_resource_text_json(value: &Value) -> Result<Option<Value>> {
    let Some(text) = value
        .get("contents")
        .and_then(Value::as_array)
        .and_then(|contents| contents.first())
        .and_then(|content| content.get("text"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    serde_json::from_str(text)
        .context("decode MCP resource text JSON")
        .map(Some)
}

fn inbox_has_unread(value: &Value) -> bool {
    value
        .get("unread_count")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0)
}

/// Compute the `(created_at_ns, id)` key of the newest unread message in an
/// inbox resource. The store already sorts ASC by `(created_at_ns, id)` but the
/// max is folded defensively rather than assuming `messages.last()`.
fn latest_message_key(value: &Value) -> Option<(i64, String)> {
    value
        .get("messages")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(message_key)
        .max()
}

fn message_key(message: &Value) -> Option<(i64, String)> {
    let created_at_ns = message.get("created_at_ns").and_then(Value::as_i64)?;
    let id = message.get("id").and_then(Value::as_str)?.to_string();
    Some((created_at_ns, id))
}

#[cfg(test)]
mod tests {
    use super::{
        AgentMailEventKind, WatchTarget, classify_notification, claude_channel_notification,
        extract_resource_text_json, inbox_has_unread, latest_message_key, parse_sse_event,
    };
    use serde_json::json;

    #[test]
    fn inbox_uri_percent_encodes_project_and_identity() {
        let target = WatchTarget {
            project: "my/project".into(),
            identity: "worker/frontend".into(),
            role: "frontend".into(),
        };

        assert_eq!(
            target.inbox_uri(),
            "agent-mail://projects/my%2Fproject/inbox?identity=worker%2Ffrontend"
        );
    }

    #[test]
    fn parses_sse_data_event() {
        let value = parse_sse_event(
            "event: message\ndata: {\"method\":\"notifications/resources/list_changed\"}",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            value,
            json!({ "method": "notifications/resources/list_changed" })
        );
    }

    #[test]
    fn classifies_inbox_update() {
        let event = classify_notification(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": {
                "uri": "agent-mail://projects/demo/inbox?identity=worker"
            }
        }))
        .unwrap();

        assert_eq!(event.kind, AgentMailEventKind::InboxUpdated);
        assert!(event.resource.is_none());
        assert_eq!(
            event.resource_uri.as_deref(),
            Some("agent-mail://projects/demo/inbox?identity=worker")
        );
    }

    #[test]
    fn claude_channel_notification_uses_supported_schema() {
        let event = classify_notification(&json!({
            "method": "notifications/resources/updated",
            "params": {
                "uri": "agent-mail://projects/demo/inbox?identity=worker"
            }
        }))
        .unwrap();

        let notification = claude_channel_notification(&event, "mail arrived");

        assert_eq!(
            notification
                .get("method")
                .and_then(|method| method.as_str()),
            Some("notifications/claude/channel")
        );
        assert_eq!(
            notification
                .get("params")
                .and_then(|params| params.get("content"))
                .and_then(|content| content.as_str()),
            Some("mail arrived")
        );
    }

    #[test]
    fn latest_message_key_folds_max_defensively() {
        let resource = json!({
            "unread_count": 3,
            "messages": [
                { "id": "m3", "created_at_ns": 30 },
                { "id": "m1", "created_at_ns": 10 },
                { "id": "m2", "created_at_ns": 30 }
            ]
        });

        // Newest is the max over (created_at_ns, id): ns 30 ties, id "m3" > "m2".
        assert_eq!(latest_message_key(&resource), Some((30, "m3".to_string())));
    }

    #[test]
    fn latest_message_key_is_none_without_messages() {
        assert_eq!(latest_message_key(&json!({ "unread_count": 0 })), None);
        assert_eq!(latest_message_key(&json!({ "messages": [] })), None);
    }

    #[test]
    fn extracts_resource_text_json() {
        let resource = extract_resource_text_json(&json!({
            "contents": [{
                "uri": "agent-mail://projects/demo/inbox?identity=worker",
                "mimeType": "application/json",
                "text": "{\"unread_count\":1}"
            }]
        }))
        .unwrap()
        .unwrap();

        assert!(inbox_has_unread(&resource));
    }
}
