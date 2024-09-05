#![feature(decl_macro, tuple_trait)]

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use database::{DataBase, Message};
use flexi_logger::{Logger, WriteMode};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use interface::routes::{self, HttpMethod};
use interface::{
    FetchLatestUpdateDateForm, FetchLatestUpdateDateResponse, FetchMessagesForm,
    FetchMessagesResponse, SendMessageForm, SendMessageResponse,
};
use tokio::net::TcpListener;
use utils::{respond_with_status, DynResult, IntoResponse, Json, State};

mod database;
mod utils;

#[derive(Clone, Default)]
struct ServerState {
    database: Arc<DataBase>,
}

#[tokio::main]
async fn main() -> DynResult<()> {
    // Set up logger.
    let _logger = Logger::try_with_str("info")
        .unwrap()
        .write_mode(WriteMode::BufferAndFlush)
        .start()
        .unwrap();

    // Set up TCP listener.
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

    let server_state = Arc::new(ServerState::default());

    loop {
        let (tcp, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let server_state = server_state.clone();
        let service =
            service_fn(move |request| handle_request(server_state.clone(), request, remote_addr));
        tokio::task::spawn(async move {
            let connection_result = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, service)
                .await;
            if let Err(err) = connection_result {
                println!("Error serving connection from address {remote_addr}: {err:?}");
            }
        });
    }
}

async fn handle_request(
    server_state: Arc<ServerState>,
    request: Request<Incoming>,
    remote_addr: SocketAddr,
) -> DynResult<hyper::Response<Full<Bytes>>> {
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
    let method: HttpMethod = request.method().into();
    let path: String = request.uri().path().into();
    log::info!("Incoming request: {method} {path:?} from {remote_addr}");
    let mut request =
        utils::UnextractedRequest::new(server_state, remote_addr, method, path, request);
    let response = match (request.method, request.path.as_str()) {
        routes::HELLO => request.handle_by(handlers::hello).await?,
        routes::SEND_MESSAGE => request.handle_by(handlers::send_message).await?,
        routes::FETCH_MESSAGES => request.handle_by(handlers::fetch_messages).await?,
        routes::FETCH_LATEST_UPDATE_DATE => {
            request
                .handle_by(handlers::fetch_latest_update_date)
                .await?
        }
        (method, path) => format!("404 Not found: {method} {path}")
            .into_response()
            .status(StatusCode::NOT_FOUND),
    };
    Ok(response.into_hyper_response())
}

mod handlers {

    use utils::ToJson;

    use super::*;

    pub async fn hello() -> DynResult<impl IntoResponse> {
        Ok("HELLO, WORLD")
    }

    pub async fn send_message(
        State(server_state): State<ServerState>,
        Json(form): Json<SendMessageForm>,
    ) -> DynResult<impl IntoResponse> {
        let message = Message {
            content: form.content.into(),
            date: Utc::now(),
        };
        server_state.database.add_message(message);
        Ok(SendMessageResponse::ok().to_json())
    }

    pub async fn fetch_messages(
        State(server_state): State<ServerState>,
        Json(form): Json<FetchMessagesForm>,
    ) -> DynResult<impl IntoResponse> {
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
        log::info!(
            "Responding fetch messages request with {} messages",
            messages.len()
        );
        Ok(FetchMessagesResponse {
            messages: messages.into(),
        }
        .to_json())
    }

    pub async fn fetch_latest_update_date(
        State(server_state): State<ServerState>,
        Json(_): Json<FetchLatestUpdateDateForm>,
    ) -> DynResult<impl IntoResponse> {
        let latest_message_date = server_state.database.latest_message_date();
        Ok(FetchLatestUpdateDateResponse {
            latest_update_date: latest_message_date,
        }
        .to_json())
    }
}
