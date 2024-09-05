use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::buf::Reader;
use bytes::{Buf, Bytes};
use chrono::Utc;
use database::{DataBase, Message};
use flexi_logger::{Logger, WriteMode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use interface::routes::{self, HttpMethod};
use interface::{
    FetchLatestUpdateDateForm, FetchLatestUpdateDateResponse, FetchMessagesForm,
    FetchMessagesResponse, SendMessageForm, SendMessageResponse,
};
use serde::de::DeserializeOwned;
use tokio::net::TcpListener;

mod database;

pub type DynError = Box<dyn std::error::Error>;
pub type DynThreadSafeError = Box<dyn std::error::Error + Send + Sync>;
pub type DynResult<T> = Result<T, DynError>;
pub type DynThreadSafeResult<T> = Result<T, DynThreadSafeError>;

#[derive(Clone, Default)]
struct ServerState {
    database: Arc<DataBase>,
}

#[tokio::main]
async fn main() -> DynThreadSafeResult<()> {
    let _logger = Logger::try_with_str("info")
        .unwrap()
        .write_mode(WriteMode::BufferAndFlush)
        .start()
        .unwrap();

    let server_state = Arc::new(ServerState::default());

    let port = {
        let port_string = env::args().nth(1);
        let port_string = port_string.as_deref().unwrap_or_else(|| {
            log::warn!("Unspecified local address, using 3000");
            "3000"
        });
        port_string.parse::<u16>().unwrap_or_else(|_| {
            log::warn!("Invalid port number {port_string:?}, using 3000");
            3000
        })
    };
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;

    if let Ok(local_addr) = listener.local_addr() {
        log::info!("Server listening on {local_addr}");
    } else {
        log::info!("Server listening on unknown address");
    }

    loop {
        let (tcp, addr) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let server_state = server_state.clone();
        let service = service_fn(move |request| handle_request(server_state.clone(), request));
        tokio::task::spawn(async move {
            let connection_result = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, service)
                .await;
            if let Err(err) = connection_result {
                println!("Error serving connection from address {addr}: {err:?}");
            }
        });
    }
}

// Half ass emulation of Axum's extractors.

pub struct Json<T: DeserializeOwned>(pub T);

impl<T: DeserializeOwned, B: Buf> TryFrom<Reader<B>> for Json<T> {
    type Error = serde_json::Error;
    fn try_from(value: Reader<B>) -> Result<Self, Self::Error> {
        serde_json::from_reader(value).map(Self)
    }
}

macro_rules! extract_json {
    ($request_body:expr) => {{
        let reader = $request_body.collect().await?.aggregate().reader();
        match reader.try_into() {
            Ok(extracted) => extracted,
            Err(_) => return Ok(respond_with_status(StatusCode::BAD_REQUEST, Bytes::new())),
        }
    }};
}

async fn handle_request(
    server_state: Arc<ServerState>,
    request: Request<Incoming>,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    if request.version() != hyper::Version::HTTP_11 {
        respond_with_status(
            StatusCode::HTTP_VERSION_NOT_SUPPORTED,
            "not HTTP/1.1, abort connection",
        );
        log::info!("Got request with unsupported HTTP version");
    }
    if let Some(upgrade) = request.headers().get("upgrade") {
        if upgrade == "websocket" {
            log::error!("Client wants to upgrade to websocket");
            respond_with_status(StatusCode::UPGRADE_REQUIRED, "WebSocket not supported yet");
        }
    }
    let method = request.method();
    let path = request.uri().path();
    log::info!("Incoming request: {method} {path:?}");
    let response = match (HttpMethod::from(method), path) {
        routes::HELLO => handle_hello().await?,
        routes::SEND_MESSAGE => {
            handle_send_message(server_state, extract_json!(request.into_body())).await?
        }
        routes::FETCH_MESSAGES => {
            handle_fetch_message(server_state, extract_json!(request.into_body())).await?
        }
        routes::FETCH_LATEST_UPDATE_DATE => {
            handle_fetch_latest_update_date(server_state, extract_json!(request.into_body()))
                .await?
        }
        (method, path) => respond_with_status(
            StatusCode::NOT_FOUND,
            format!("404 Not found: {method} {path}"),
        ),
    };
    Ok(response)
}

async fn handle_hello() -> DynThreadSafeResult<Response<Full<Bytes>>> {
    Ok(respond("HELLO, WORLD"))
}

async fn handle_send_message(
    server_state: Arc<ServerState>,
    Json(form): Json<SendMessageForm>,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    let message = Message {
        content: form.content.into(),
        date: Utc::now(),
    };
    log::info!("message: {message:?}");
    server_state.database.add_message(message);
    let response_json = serde_json::to_string(&SendMessageResponse::ok()).unwrap();
    Ok(respond(response_json))
}

async fn handle_fetch_message(
    server_state: Arc<ServerState>,
    Json(form): Json<FetchMessagesForm>,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    log::info!("form: {form:?}");
    let count = u32::min(form.max_count, 50);
    let messages: Vec<interface::Message> = server_state
        .database
        .latest_messages(count as usize)
        .into_iter()
        .filter(|message| {
            // FIXME: optimize this with the assumption of messages being ordered chronologically.
            form.since
                .map(|since| message.date >= since)
                .unwrap_or(true)
        })
        .map(|message| interface::Message {
            content: message.content.as_ref().to_owned().into(),
            date: message.date,
        })
        .collect();
    let response = FetchMessagesResponse {
        messages: messages.into(),
    };
    let response_json = serde_json::to_string(&response).unwrap();
    log::info!(
        "Someone fetched {} messages since {:?}, giving them {} messages",
        form.max_count,
        form.since,
        response.messages.len(),
    );
    Ok(respond(response_json))
}

async fn handle_fetch_latest_update_date(
    server_state: Arc<ServerState>,
    Json(_form): Json<FetchLatestUpdateDateForm>,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    let latest_message_date = server_state.database.latest_message_date();
    let response = FetchLatestUpdateDateResponse {
        latest_update_date: latest_message_date,
    };
    let response_json = serde_json::to_string(&response).unwrap();
    log::info!("response: {response_json:?}",);
    Ok(respond(response_json))
}

fn respond(bytes: impl Into<Bytes>) -> Response<Full<Bytes>> {
    respond_with_status(StatusCode::OK, bytes)
}

fn respond_with_status(status: StatusCode, bytes: impl Into<Bytes>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(bytes.into()))
        .unwrap()
}
