use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    sync::Arc,
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};

use crate::{
    domain::{SendMessage, StartParticipant},
    error::{AppError, Result},
    http::{AppState, authorize},
    id,
};

const PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
const MCP_SESSION_ID: &str = "mcp-session-id";
const MCP_PROTOCOL_VERSION: &str = "mcp-protocol-version";

#[derive(Clone, Default)]
pub struct McpHub {
    sessions: Arc<Mutex<HashMap<String, McpSession>>>,
}

#[derive(Default)]
struct McpSession {
    identity: Option<String>,
    role: Option<String>,
    initialized: bool,
    protocol_version: String,
    subscriptions: HashSet<String>,
    stream: Option<mpsc::UnboundedSender<Value>>,
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

pub async fn mcp_get(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Err(err) = authorize_mcp(&state, &headers) {
        return err.into_response();
    }
    let Some(session_id) = header_text(&headers, MCP_SESSION_ID) else {
        return (StatusCode::BAD_REQUEST, "missing MCP-Session-Id").into_response();
    };
    if !accepts(&headers, "text/event-stream") {
        return (StatusCode::BAD_REQUEST, "missing Accept: text/event-stream").into_response();
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut sessions = state.mcp.sessions.lock().await;
        let Some(session) = sessions.get_mut(&session_id) else {
            return (StatusCode::NOT_FOUND, "unknown MCP session").into_response();
        };
        if let Err(err) = validate_session_protocol(session, &headers) {
            return err.into_response();
        }
        session.stream = Some(tx);
    }

    let stream = async_stream::stream! {
        yield Ok::<Event, Infallible>(Event::default().event("message").data(""));
        while let Some(message) = rx.recv().await {
            match Event::default().json_data(message) {
                Ok(event) => yield Ok(event),
                Err(_) => yield Ok(Event::default().event("message").data("{}")),
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub async fn mcp_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> Response {
    if let Err(err) = authorize_mcp(&state, &headers) {
        return err.into_response();
    }
    if let Err(err) = validate_rpc_shape(&input) {
        return json_rpc_http(None, rpc_error(None, err.code, &err.message));
    }
    let request: RpcRequest = match serde_json::from_value(input) {
        Ok(value) => value,
        Err(err) => return json_rpc_http(None, rpc_error(None, -32700, &err.to_string())),
    };

    let session_id = header_text(&headers, MCP_SESSION_ID);
    if request.method != "initialize" {
        let Some(id) = session_id.as_deref() else {
            return (StatusCode::BAD_REQUEST, "missing MCP-Session-Id").into_response();
        };
        let sessions = state.mcp.sessions.lock().await;
        let Some(session) = sessions.get(id) else {
            return (StatusCode::NOT_FOUND, "unknown MCP session").into_response();
        };
        if let Err(err) = validate_session_protocol(session, &headers) {
            return err.into_response();
        }
    }

    let is_notification = request.id.is_none();
    let result = handle_rpc(&state, session_id.as_deref(), &request).await;
    match result {
        Ok(_) if is_notification => StatusCode::ACCEPTED.into_response(),
        Ok(Some((reply, new_session_id))) => {
            let mut response = json_rpc_http(request.id.clone(), reply);
            if let Some(id) = new_session_id {
                let Ok(value) = HeaderValue::from_str(&id) else {
                    return json_rpc_http(
                        request.id.clone(),
                        rpc_error(request.id, -32000, "invalid generated MCP session id"),
                    );
                };
                response.headers_mut().insert(MCP_SESSION_ID, value);
            }
            response
        }
        Ok(None) => json_rpc_http(request.id.clone(), rpc_result(request.id, json!({}))),
        Err(err) => json_rpc_http(
            request.id.clone(),
            rpc_error(request.id, -32000, &err.to_string()),
        ),
    }
}

async fn handle_rpc(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    request: &RpcRequest,
) -> Result<Option<(Value, Option<String>)>> {
    match request.method.as_str() {
        "initialize" => {
            let protocol_version = requested_protocol_version(&request.params);
            let id = id::session_id().map_err(|err| AppError::BadRequest(err.to_string()))?;
            state.mcp.sessions.lock().await.insert(
                id.clone(),
                McpSession {
                    protocol_version: protocol_version.clone(),
                    ..McpSession::default()
                },
            );
            Ok(Some((
                rpc_result(
                    request.id.clone(),
                    json!({
                        "protocolVersion": protocol_version,
                        "capabilities": {
                            "tools": {},
                            "resources": { "subscribe": true, "listChanged": true }
                        },
                        "serverInfo": {
                            "name": "agent-mail",
                            "title": "Agent Mail",
                            "version": env!("CARGO_PKG_VERSION")
                        },
                        "instructions": "Use tools only for mutations. Read inboxes and messages through MCP resources; resource reads do not mark mail read."
                    }),
                ),
                Some(id),
            )))
        }
        "notifications/initialized" => {
            if let Some(id) = session_id {
                let mut sessions = state.mcp.sessions.lock().await;
                let session = sessions
                    .get_mut(id)
                    .ok_or_else(|| AppError::NotFound("unknown MCP session".into()))?;
                session.initialized = true;
            }
            Ok(None)
        }
        "ping" => Ok(Some((rpc_result(request.id.clone(), json!({})), None))),
        "tools/list" => Ok(Some((
            rpc_result(request.id.clone(), json!({ "tools": tool_list() })),
            None,
        ))),
        "tools/call" => {
            let name = required_string(&request.params, "name")?;
            let args = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let output = call_tool(state, session_id, &name, args).await?;
            Ok(Some((
                rpc_result(
                    request.id.clone(),
                    json!({
                        "content": [{
                            "type": "text",
                            "text": json_text(&output)?
                        }]
                    }),
                ),
                None,
            )))
        }
        "resources/list" => Ok(Some((
            rpc_result(request.id.clone(), resource_list(state, session_id).await?),
            None,
        ))),
        "resources/templates/list" => Ok(Some((
            rpc_result(
                request.id.clone(),
                json!({ "resourceTemplates": resource_templates() }),
            ),
            None,
        ))),
        "resources/read" => {
            let uri = required_string(&request.params, "uri")?;
            let output = read_resource(state, &uri).await?;
            Ok(Some((rpc_result(request.id.clone(), output), None)))
        }
        "resources/subscribe" => {
            let uri = required_string(&request.params, "uri")?;
            validate_subscribable_resource(&uri)?;
            let id = require_session_id(session_id)?;
            let mut sessions = state.mcp.sessions.lock().await;
            let session = sessions
                .get_mut(id)
                .ok_or_else(|| AppError::NotFound("unknown MCP session".into()))?;
            session.subscriptions.insert(uri);
            Ok(Some((rpc_result(request.id.clone(), json!({})), None)))
        }
        "resources/unsubscribe" => {
            let uri = required_string(&request.params, "uri")?;
            let id = require_session_id(session_id)?;
            let mut sessions = state.mcp.sessions.lock().await;
            let session = sessions
                .get_mut(id)
                .ok_or_else(|| AppError::NotFound("unknown MCP session".into()))?;
            session.subscriptions.remove(&uri);
            Ok(Some((rpc_result(request.id.clone(), json!({})), None)))
        }
        _ => Ok(Some((
            rpc_error(
                request.id.clone(),
                -32601,
                &format!("method not found: {}", request.method),
            ),
            None,
        ))),
    }
}

async fn call_tool(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    name: &str,
    args: Value,
) -> Result<Value> {
    match name {
        "agent_mail_start" => {
            let role = required_string(&args, "role")?;
            let session = state
                .store
                .start(StartParticipant {
                    identity: None,
                    role: role.clone(),
                })
                .await?;
            let id = require_session_id(session_id)?;
            let mut sessions = state.mcp.sessions.lock().await;
            let mcp_session = sessions
                .get_mut(id)
                .ok_or_else(|| AppError::NotFound("unknown MCP session".into()))?;
            if let Some(existing_role) = &mcp_session.role
                && existing_role != &session.role
            {
                return Err(AppError::Conflict(format!(
                    "MCP session already started as role {existing_role:?}"
                )));
            }
            mcp_session.identity = Some(session.identity.clone());
            mcp_session.role = Some(session.role.clone());
            json_value(session)
        }
        "agent_mail_project_add" => {
            let alias = required_string(&args, "alias")?;
            let root = optional_string(&args, "root").unwrap_or_default();
            let project = state.store.add_project(&alias, &root).await?;
            notify_resource(state, "agent-mail://projects").await;
            notify_list_changed(state).await;
            json_value(project)
        }
        "agent_mail_send" => {
            let (identity, _) = session_participant(state, session_id).await?;
            let project = required_string(&args, "project")?;
            let to = required_string(&args, "to")?;
            let subject = required_string(&args, "subject")?;
            let body = required_string(&args, "body")?;
            let message = state
                .store
                .send(SendMessage {
                    sender_identity: identity,
                    project,
                    to_kind: String::new(),
                    to,
                    subject,
                    body,
                })
                .await?;
            notify_matching_inboxes(state, &message.project).await;
            json_value(message)
        }
        "agent_mail_mark_read" => {
            let (identity, _) = session_participant(state, session_id).await?;
            let project = required_string(&args, "project")?;
            let mail_id = required_string(&args, "mail_id")?;
            state.store.mark_read(&project, &mail_id, &identity).await?;
            notify_resource(state, &inbox_uri(&project, &identity)).await;
            notify_matching_message_resources(state, &project, &mail_id).await;
            Ok(json!({ "marked_read": mail_id }))
        }
        _ => Err(AppError::BadRequest(format!("unknown tool: {name}"))),
    }
}

async fn resource_list(state: &Arc<AppState>, session_id: Option<&str>) -> Result<Value> {
    let mut resources = vec![json!({
        "uri": "agent-mail://projects",
        "name": "projects",
        "title": "Agent Mail Projects",
        "mimeType": "application/json"
    })];
    if let Ok((identity, _)) = session_participant(state, session_id).await {
        for project in state.store.projects().await? {
            resources.push(json!({
                "uri": inbox_uri(&project.alias, &identity),
                "name": format!("{} inbox", project.alias),
                "title": format!("{} inbox", project.alias),
                "mimeType": "application/json"
            }));
        }
    }
    Ok(json!({ "resources": resources }))
}

async fn read_resource(state: &Arc<AppState>, uri: &str) -> Result<Value> {
    let value = if uri == "agent-mail://projects" {
        json!({ "projects": state.store.projects().await? })
    } else if let Some((project, identity)) = parse_inbox_uri(uri)? {
        json_value(state.store.inbox(&project, &identity).await?)?
    } else if let Some((project, mail_id, identity)) = parse_message_uri(uri)? {
        json_value(state.store.message(&project, &mail_id, &identity).await?)?
    } else {
        return Err(AppError::NotFound(format!("unknown resource URI {uri:?}")));
    };
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": json_text(&value)?
        }]
    }))
}

fn tool_list() -> Value {
    json!([
        {
            "name": "agent_mail_start",
            "description": "Start this MCP session as an Agent Mail participant with the given role. The generated identity is bound to this MCP session.",
            "inputSchema": {
                "type": "object",
                "properties": { "role": { "type": "string" } },
                "required": ["role"],
                "additionalProperties": false
            }
        },
        {
            "name": "agent_mail_project_add",
            "description": "Create or update an Agent Mail project namespace. Re-adding the same alias updates root metadata.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "alias": { "type": "string" },
                    "root": { "type": "string" }
                },
                "required": ["alias"],
                "additionalProperties": false
            }
        },
        {
            "name": "agent_mail_send",
            "description": "Send mail from this MCP session identity. Recipient is inferred as identity, role, or all-agents broadcast.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "to": { "type": "string" },
                    "subject": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["project", "to", "subject", "body"],
                "additionalProperties": false
            }
        },
        {
            "name": "agent_mail_mark_read",
            "description": "Mark delivered mail read for this MCP session identity.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "mail_id": { "type": "string" }
                },
                "required": ["project", "mail_id"],
                "additionalProperties": false
            }
        }
    ])
}

fn resource_templates() -> Value {
    json!([
        {
            "uriTemplate": "agent-mail://projects/{alias}/inbox?identity={identity}",
            "name": "project-inbox",
            "title": "Agent Mail project inbox",
            "description": "Unread Agent Mail for an identity in a project.",
            "mimeType": "application/json"
        },
        {
            "uriTemplate": "agent-mail://projects/{alias}/messages/{mail_id}?identity={identity}",
            "name": "project-message",
            "title": "Agent Mail message",
            "description": "Full Agent Mail message body for delivered mail.",
            "mimeType": "application/json"
        }
    ])
}

async fn session_participant(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<(String, String)> {
    let id = require_session_id(session_id)?;
    let sessions = state.mcp.sessions.lock().await;
    let session = sessions
        .get(id)
        .ok_or_else(|| AppError::NotFound("unknown MCP session".into()))?;
    match (&session.identity, &session.role) {
        (Some(identity), Some(role)) => Ok((identity.clone(), role.clone())),
        _ => Err(AppError::BadRequest(
            "call agent_mail_start before using session-scoped tools".into(),
        )),
    }
}

async fn notify_matching_inboxes(state: &Arc<AppState>, project: &str) {
    let subscriptions = subscribed_uris(state).await;
    for uri in subscriptions {
        if let Ok(Some((sub_project, _))) = parse_inbox_uri(&uri)
            && sub_project == project
        {
            notify_resource(state, &uri).await;
        }
    }
}

async fn notify_matching_message_resources(state: &Arc<AppState>, project: &str, mail_id: &str) {
    let subscriptions = subscribed_uris(state).await;
    for uri in subscriptions {
        if let Ok(Some((sub_project, sub_mail_id, _))) = parse_message_uri(&uri)
            && sub_project == project
            && sub_mail_id == mail_id
        {
            notify_resource(state, &uri).await;
        }
    }
}

async fn subscribed_uris(state: &Arc<AppState>) -> HashSet<String> {
    let sessions = state.mcp.sessions.lock().await;
    sessions
        .values()
        .flat_map(|session| session.subscriptions.iter().cloned())
        .collect()
}

async fn notify_resource(state: &Arc<AppState>, uri: &str) {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": { "uri": uri }
    });
    let sessions = state.mcp.sessions.lock().await;
    for session in sessions.values() {
        if (session.subscriptions.contains(uri) || uri == "agent-mail://projects")
            && let Some(stream) = &session.stream
        {
            let _ = stream.send(notification.clone());
        }
    }
}

async fn notify_list_changed(state: &Arc<AppState>) {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/list_changed"
    });
    let sessions = state.mcp.sessions.lock().await;
    for session in sessions.values() {
        if let Some(stream) = &session.stream {
            let _ = stream.send(notification.clone());
        }
    }
}

fn validate_subscribable_resource(uri: &str) -> Result<()> {
    if uri == "agent-mail://projects"
        || parse_inbox_uri(uri)?.is_some()
        || parse_message_uri(uri)?.is_some()
    {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "resource is not subscribable: {uri}"
        )))
    }
}

fn parse_inbox_uri(uri: &str) -> Result<Option<(String, String)>> {
    let Some(rest) = uri.strip_prefix("agent-mail://projects/") else {
        return Ok(None);
    };
    let Some((project, query)) = rest.split_once("/inbox?") else {
        return Ok(None);
    };
    let identity = query_param(query, "identity")
        .ok_or_else(|| AppError::BadRequest("inbox resource requires identity query".into()))?;
    Ok(Some((decode_component(project)?, identity)))
}

fn parse_message_uri(uri: &str) -> Result<Option<(String, String, String)>> {
    let Some(rest) = uri.strip_prefix("agent-mail://projects/") else {
        return Ok(None);
    };
    let Some((project, rest)) = rest.split_once("/messages/") else {
        return Ok(None);
    };
    let Some((mail_id, query)) = rest.split_once('?') else {
        return Err(AppError::BadRequest(
            "message resource requires identity query".into(),
        ));
    };
    let identity = query_param(query, "identity")
        .ok_or_else(|| AppError::BadRequest("message resource requires identity query".into()))?;
    Ok(Some((
        decode_component(project)?,
        decode_component(mail_id)?,
        identity,
    )))
}

fn inbox_uri(project: &str, identity: &str) -> String {
    format!(
        "agent-mail://projects/{}/inbox?identity={}",
        encode_component(project),
        encode_component(identity)
    )
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key == name {
            decode_component(value).ok()
        } else {
            None
        }
    })
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

fn decode_component(value: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let raw = value.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            if i + 2 >= raw.len() {
                return Err(AppError::BadRequest("invalid percent encoding".into()));
            }
            let hex = std::str::from_utf8(&raw[i + 1..i + 3])
                .map_err(|_| AppError::BadRequest("invalid percent encoding".into()))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| AppError::BadRequest("invalid percent encoding".into()))?;
            bytes.push(byte);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(bytes).map_err(|_| AppError::BadRequest("invalid utf-8".into()))
}

fn required_string(params: &Value, name: &str) -> Result<String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest(format!("missing string parameter {name:?}")))
}

fn optional_string(params: &Value, name: &str) -> Option<String> {
    params.get(name).and_then(Value::as_str).map(str::to_string)
}

fn json_value<T: Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value)
        .map_err(|err| AppError::Internal(format!("serialize response: {err}")))
}

fn json_text(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|err| AppError::Internal(format!("serialize response: {err}")))
}

fn require_session_id(session_id: Option<&str>) -> Result<&str> {
    session_id.ok_or_else(|| AppError::BadRequest("missing MCP session".into()))
}

fn rpc_result(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn json_rpc_http(_id: Option<Value>, body: Value) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn accepts(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|part| part.trim().starts_with(expected))
        })
}

fn authorize_mcp(state: &AppState, headers: &HeaderMap) -> Result<()> {
    validate_origin(state, headers)?;
    authorize(state, headers)
}

fn validate_origin(state: &AppState, headers: &HeaderMap) -> Result<()> {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return Ok(());
    };
    if state
        .allowed_origins
        .iter()
        .any(|allowed| allowed == origin)
        || origin == "http://127.0.0.1"
        || origin == "http://localhost"
        || origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
    {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

struct RpcShapeError {
    code: i64,
    message: String,
}

fn validate_rpc_shape(input: &Value) -> std::result::Result<(), RpcShapeError> {
    let Some(object) = input.as_object() else {
        return Err(RpcShapeError {
            code: -32600,
            message: "JSON-RPC request must be an object".into(),
        });
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(RpcShapeError {
            code: -32600,
            message: "JSON-RPC request must include jsonrpc \"2.0\"".into(),
        });
    }
    if let Some(id) = object.get("id")
        && !(id.is_string() || id.is_number())
    {
        return Err(RpcShapeError {
            code: -32600,
            message: "JSON-RPC id must be a string or number".into(),
        });
    }
    Ok(())
}

fn requested_protocol_version(params: &Value) -> String {
    params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
        .unwrap_or(PROTOCOL_VERSION)
        .to_string()
}

fn validate_session_protocol(session: &McpSession, headers: &HeaderMap) -> Result<()> {
    let Some(version) = header_text(headers, MCP_PROTOCOL_VERSION) else {
        return Ok(());
    };
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&version.as_str())
        && (session.protocol_version.is_empty() || version == session.protocol_version)
    {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "unsupported MCP-Protocol-Version for session".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_component, encode_component, parse_inbox_uri, parse_message_uri};

    #[test]
    fn uri_component_round_trips_reserved_bytes() {
        let raw = "project/with space?and=value";
        let encoded = encode_component(raw);

        assert_eq!(encoded, "project%2Fwith%20space%3Fand%3Dvalue");
        assert_eq!(decode_component(&encoded).unwrap(), raw);
    }

    #[test]
    fn inbox_uri_parser_decodes_identity_query() {
        let parsed =
            parse_inbox_uri("agent-mail://projects/my.project/inbox?identity=worker%2Ffrontend")
                .unwrap()
                .unwrap();

        assert_eq!(
            parsed,
            ("my.project".to_string(), "worker/frontend".to_string())
        );
    }

    #[test]
    fn message_uri_parser_decodes_message_id_and_identity() {
        let parsed = parse_message_uri(
            "agent-mail://projects/my.project/messages/mail%2F1?identity=worker%2Ffrontend",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            parsed,
            (
                "my.project".to_string(),
                "mail/1".to_string(),
                "worker/frontend".to_string()
            )
        );
    }

    #[test]
    fn uri_component_rejects_invalid_percent_encoding() {
        assert!(decode_component("%").is_err());
        assert!(decode_component("%xy").is_err());
    }
}
