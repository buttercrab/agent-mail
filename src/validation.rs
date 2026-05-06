use crate::domain::BROADCAST_RECIPIENT;
use crate::error::{AppError, Result};

pub fn alias(value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(AppError::BadRequest(
            "project alias may contain only letters, numbers, dot, underscore, and dash".into(),
        ));
    }
    Ok(())
}

pub fn identity(value: &str) -> Result<()> {
    address_path(value, "identity")
}

pub fn role(value: &str) -> Result<()> {
    if value.trim() == BROADCAST_RECIPIENT {
        return Err(AppError::BadRequest(
            "all-agents is a reserved broadcast address, not a role".into(),
        ));
    }
    address_path(value, "role")
}

pub fn recipient(value: &str) -> Result<()> {
    address_path(value, "recipient")
}

fn address_path(value: &str, field: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!("{field} is required")));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
    {
        return Err(AppError::BadRequest(format!(
            "{field} may contain only letters, numbers, dot, dash, underscore, and slash"
        )));
    }
    if trimmed.contains("//") || trimmed.starts_with('/') || trimmed.ends_with('/') {
        return Err(AppError::BadRequest(format!(
            "{field} may not contain empty slash-separated segments"
        )));
    }
    Ok(())
}
