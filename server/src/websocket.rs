#![expect(unused_imports)]

use std::net::SocketAddr;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper_tungstenite::HyperWebsocket;
use tokio_tungstenite::tungstenite::Message as WebsocketMessage;

use crate::utils::DynResult;

pub fn is_upgrade_request(request: &hyper::Request<Incoming>) -> bool {
    hyper_tungstenite::is_upgrade_request(request)
}

pub async fn handle(
    remote_addr: SocketAddr,
    mut request: hyper::Request<Incoming>,
) -> DynResult<hyper::Response<Full<Bytes>>> {
    let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;
    tokio::spawn(async move {
        if let Err(e) = serve_websocket(remote_addr, websocket).await {
            log::error!("Error serving websocket with {remote_addr}: {e}");
        }
    });
    Ok(response)
}

async fn serve_websocket(remote_addr: SocketAddr, websocket: HyperWebsocket) -> DynResult<()> {
    log::info!("Starting websocket connection with {remote_addr}");
    let mut websocket = websocket.await?;
    websocket.send("hello".into()).await?;
    // while let Some(message) = websocket.next().await {
    // }
    Ok(())
}
