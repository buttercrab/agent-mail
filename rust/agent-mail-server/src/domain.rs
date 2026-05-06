use serde::{Deserialize, Serialize};

pub const BROADCAST_RECIPIENT: &str = "all-agents";

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub identity: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Participant {
    pub identity: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub alias: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub root: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub id: String,
    pub project: String,
    pub sender_identity: String,
    pub sender_role: String,
    pub recipient_kind: String,
    pub recipient: String,
    pub subject: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub body: String,
    pub created_at: String,
    pub created_at_ns: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub read_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inbox {
    pub project: String,
    pub identity: String,
    pub role: String,
    pub unread_count: usize,
    pub messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
pub struct StartParticipant {
    pub identity: Option<String>,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct AddProject {
    pub alias: String,
    #[serde(default)]
    pub root: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessage {
    pub sender_identity: String,
    pub project: String,
    #[serde(default)]
    pub to_kind: String,
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct MarkRead {
    pub identity: String,
}
