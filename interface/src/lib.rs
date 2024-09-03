use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageForm {
    pub content: Box<str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub ok: bool,
}

impl SendMessageResponse {
    pub const fn ok() -> Self {
        Self { ok: true }
    }
    pub const fn not_ok() -> Self {
        Self { ok: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub content: Box<str>,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchMessagesForm {
    /// Maximum number of recent messages to fetch.
    pub max_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchMessagesResponse {
    pub messages: Box<[Message]>,
}
