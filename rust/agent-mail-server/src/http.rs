use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    domain::{AddProject, MarkRead, SendMessage, StartParticipant},
    error::{AppError, Result},
    mcp::McpHub,
    store::Store,
};

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub token: String,
    pub allowed_origins: Vec<String>,
    pub mcp: McpHub,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/mcp", get(crate::mcp::mcp_get).post(crate::mcp::mcp_post))
        .route("/v1/participants/start", post(start_participant))
        .route("/v1/participants", get(list_participants))
        .route("/v1/projects", get(list_projects).post(add_project))
        .route("/v1/messages", post(send_message))
        .route(
            "/v1/projects/{project}/participants/{identity}/inbox",
            get(read_inbox),
        )
        .route(
            "/v1/projects/{project}/messages/{mail_id}",
            get(read_message),
        )
        .route(
            "/v1/projects/{project}/messages/{mail_id}/read",
            post(mark_read),
        )
        .with_state(Arc::new(state))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "ok": true })))
}

async fn start_participant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<StartParticipant>,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(state.store.start(input).await?))
}

async fn list_participants(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(
        json!({ "participants": state.store.participants().await? }),
    ))
}

async fn add_project(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<AddProject>,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(
        state.store.add_project(&input.alias, &input.root).await?,
    ))
}

async fn list_projects(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(json!({ "projects": state.store.projects().await? })))
}

async fn send_message(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<SendMessage>,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(state.store.send(input).await?))
}

async fn read_inbox(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((project, identity)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(state.store.inbox(&project, &identity).await?))
}

#[derive(Deserialize)]
struct MessageQuery {
    identity: String,
}

async fn read_message(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((project, mail_id)): Path<(String, String)>,
    Query(query): Query<MessageQuery>,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    Ok(Json(
        state
            .store
            .message(&project, &mail_id, &query.identity)
            .await?,
    ))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((project, mail_id)): Path<(String, String)>,
    Json(input): Json<MarkRead>,
) -> Result<impl IntoResponse> {
    authorize(&state, &headers)?;
    state
        .store
        .mark_read(&project, &mail_id, &input.identity)
        .await?;
    Ok(Json(json!({ "marked_read": mail_id })))
}

pub(crate) fn authorize(state: &AppState, headers: &HeaderMap) -> Result<()> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(AppError::Unauthorized);
    };
    let Ok(text) = value.to_str() else {
        return Err(AppError::Unauthorized);
    };
    if text == format!("Bearer {}", state.token) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
