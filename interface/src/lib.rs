use std::fmt::{self, Debug, Display};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// HTTP Methods.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Connect,
    Patch,
    Trace,
    /// Anything other than the common 9 HTTP methods.
    Unknown,
}

impl Debug for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Head => write!(f, "HEAD"),
            HttpMethod::Options => write!(f, "OPTIONS"),
            HttpMethod::Connect => write!(f, "CONNECT"),
            HttpMethod::Patch => write!(f, "PATCH"),
            HttpMethod::Trace => write!(f, "TRACE"),
            HttpMethod::Unknown => write!(f, "{{UNKNOWN}}"),
        }
    }
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl From<&hyper::Method> for HttpMethod {
    fn from(value: &hyper::Method) -> Self {
        match *value {
            hyper::Method::GET => Self::Get,
            hyper::Method::POST => Self::Post,
            hyper::Method::PUT => Self::Put,
            hyper::Method::DELETE => Self::Delete,
            hyper::Method::HEAD => Self::Head,
            hyper::Method::OPTIONS => Self::Options,
            hyper::Method::CONNECT => Self::Connect,
            hyper::Method::PATCH => Self::Patch,
            hyper::Method::TRACE => Self::Trace,
            _ => Self::Unknown, // FIXME: Not sure if this is ok.
        }
    }
}

impl From<hyper::Method> for HttpMethod {
    fn from(value: hyper::Method) -> Self {
        <Self as From<&hyper::Method>>::from(&value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownHttpMethod;
impl Display for UnknownHttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Trying to convert `HttpMethod::Unknown` to `hyper::Method`"
        )
    }
}
impl std::error::Error for UnknownHttpMethod {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }
    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl TryFrom<HttpMethod> for hyper::Method {
    type Error = UnknownHttpMethod;
    fn try_from(value: HttpMethod) -> Result<Self, Self::Error> {
        match value {
            HttpMethod::Get => Ok(hyper::Method::GET),
            HttpMethod::Post => Ok(hyper::Method::POST),
            HttpMethod::Put => Ok(hyper::Method::PUT),
            HttpMethod::Delete => Ok(hyper::Method::DELETE),
            HttpMethod::Head => Ok(hyper::Method::HEAD),
            HttpMethod::Options => Ok(hyper::Method::OPTIONS),
            HttpMethod::Connect => Ok(hyper::Method::CONNECT),
            HttpMethod::Patch => Ok(hyper::Method::PATCH),
            HttpMethod::Trace => Ok(hyper::Method::TRACE),
            HttpMethod::Unknown => Err(UnknownHttpMethod),
        }
    }
}

pub mod routes {
    use super::HttpMethod;

    // FIXME: Maybe tuples aren't the best choice here.
    pub const HELLO: (HttpMethod, &str) = (HttpMethod::Get, "/hello");
    pub const SEND_MESSAGE: (HttpMethod, &str) = (HttpMethod::Post, "/send_message");
    pub const FETCH_MESSAGES: (HttpMethod, &str) = (HttpMethod::Get, "/fetch_messages");
    pub const FETCH_LATEST_UPDATE_DATE: (HttpMethod, &str) =
        (HttpMethod::Get, "/fetch_latest_update_date");
    pub const WS: (HttpMethod, &str) = (HttpMethod::Get, "/ws");
}

pub const EXPECTED_RESPONSE_TO_HELLO: &str = "HELLO, WORLD";

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
    /// Earliest date of messages to fetch.
    /// This and `max_count` both apply at the same time.
    pub since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchMessagesResponse {
    pub messages: Box<[Message]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchLatestUpdateDateForm {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchLatestUpdateDateResponse {
    pub latest_update_date: Option<DateTime<Utc>>,
}
