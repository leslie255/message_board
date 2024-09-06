#![feature(decl_macro, tuple_trait, never_type)]

/// Emulates a data base, will swap out with a real one later.
mod database;

/// Axum-style HTTP request handling library.
mod utils;

/// Handling of HTTP requests.
mod handlers;

/// Manages everything Websocket.
mod websocket;

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
use interface::{
    routes, FetchLatestUpdateDateForm, FetchLatestUpdateDateResponse, FetchMessagesForm,
    FetchMessagesResponse, HttpMethod, SendMessageForm, SendMessageResponse,
};
use tokio::net::TcpListener;
use utils::{respond_with_status, DynResult, IntoResponse, Json, State, ToJson};

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
        let (tcp_stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(tcp_stream);
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
    if websocket::is_upgrade_request(&request) {
        let response = websocket::upgrade(remote_addr, request).await?;
        return Ok(response.into_response().into_hyper_response());
    }
    let method: HttpMethod = request.method().into();
    let path: String = request.uri().path().into();
    log::info!("Incoming request: {method} {path:?} from {remote_addr}");
    let mut request =
        utils::UnextractedRequest::new(server_state, remote_addr, method, path, request);
    let response = match (request.method, request.path.trim_end_matches('/')) {
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
