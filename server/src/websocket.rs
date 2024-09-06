use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use hyper::body::Incoming;
use hyper_tungstenite::HyperWebsocket;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::utils::{DynResult, IntoResponse};

pub fn is_upgrade_request(request: &hyper::Request<Incoming>) -> bool {
    let path_is_ws = matches!(request.uri().path(), "/ws" | "/ws/");
    path_is_ws && hyper_tungstenite::is_upgrade_request(request)
    // use hyper::{header::HeaderValue, HeaderMap}
    // fn header_contains(header: &HeaderMap<HeaderValue>, key: &str, value: &str) -> bool {
    //     header.get(key).and_then(|val| val.to_str().ok()) == Some(value)
    // }
    // let path = request.uri().path();
    // let headers = request.headers();
    // (path == "/ws" || path == "/ws/")
    //     && (header_contains(headers, "Connection", "Upgrade")
    //         && header_contains(headers, "Upgrade", "websocket"))
}

pub async fn upgrade(
    remote_addr: SocketAddr,
    mut request: hyper::Request<Incoming>,
) -> DynResult<impl IntoResponse> {
    let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;
    tokio::spawn(async move {
        serve_websocket(remote_addr, websocket)
            .await
            .unwrap_or_else(|e| log::error!("Error serving websocket: {e}"));
    });
    Ok(response)
}

async fn serve_websocket(remote_addr: SocketAddr, websocket: HyperWebsocket) -> DynResult<()> {
    log::info!("Started websocket connection with {remote_addr}");
    let mut websocket = websocket.await?;
    while let Some(message) = websocket.next().await {
        match message? {
            WsMessage::Text(s) if s.as_str() == "hello" => {
                websocket
                    .send(WsMessage::Text("HELLO, WORLD".into()))
                    .await?;
            }
            WsMessage::Text(text) => {
                log::info!("Websocket text message from {remote_addr}: {text:?}");
                websocket.send(WsMessage::Text(text)).await?;
            }
            WsMessage::Binary(_) => { // Nothing to do with it.
                log::info!("Websocket binary from {remote_addr}");
            }
            WsMessage::Ping(message) => {
                log::debug!("Websocket ping from {remote_addr} with message {message:?}")
            }
            WsMessage::Pong(message) => {
                log::debug!("Websocket pong from {remote_addr} with message {message:?}")
            }
            WsMessage::Close(Some(message)) => {
                log::info!(
                    "Websocket connection closed with {remote_addr} with a message {message:?}"
                );
            }
            WsMessage::Close(None) => {
                log::info!("Websocket connection closed with {remote_addr}");
            }
            WsMessage::Frame(_) => { // Nothing to do with it.
            }
        }
    }
    todo!()
}
