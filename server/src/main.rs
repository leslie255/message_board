use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{Buf, Bytes};
use chrono::Utc;
use database::{DataBase, Message};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::{TokioIo, TokioTimer};
use interface::{FetchMessagesForm, FetchMessagesResponse, SendMessageForm, SendMessageResponse};
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
    let server_state = Arc::new(ServerState::default());

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 3000))).await?;

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

async fn handle_request(
    server_state: Arc<ServerState>,
    request: Request<Incoming>,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    if request.version() != hyper::Version::HTTP_11 {
        reponse("not HTTP/1.1, abort connection");
    }
    let method = request.method();
    let path = request.uri().path();
    match (method, path) {
        (&Method::GET, "/hello") => handle_hello().await,
        (&Method::POST, "/send_message") => {
            handle_send_message(server_state, request.into_body()).await
        }
        (&Method::GET, "/fetch_messages") => {
            handle_fetch_message(server_state, request.into_body()).await
        }
        (method, path) => Ok(Response::new(Full::new(Bytes::from(format!(
            "404 NOT FOUND: {method} {path:?}"
        ))))),
    }
}

async fn handle_hello() -> DynThreadSafeResult<Response<Full<Bytes>>> {
    Ok(reponse("HELLO, WORLD"))
}

async fn handle_send_message(
    server_state: Arc<ServerState>,
    body: Incoming,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    let Ok(send_message_form) = read_request_body::<SendMessageForm>(body).await else {
        return Ok(reponse(
            serde_json::to_string(&SendMessageResponse::not_ok()).unwrap(),
        ));
    };
    let message = Message {
        content: send_message_form.content.into(),
        date: Utc::now(),
    };
    println!("Someone sent: {:?}", message);
    server_state.database.add_message(message);
    let response_json = serde_json::to_string(&SendMessageResponse::ok()).unwrap();
    Ok(reponse(response_json))
}

async fn handle_fetch_message(
    server_state: Arc<ServerState>,
    body: Incoming,
) -> DynThreadSafeResult<Response<Full<Bytes>>> {
    let Ok(fetch_message_form) = read_request_body::<FetchMessagesForm>(body).await else {
        return Ok(reponse("invalid /fetch_message request"));
    };
    let count = u32::min(fetch_message_form.max_count, 50);
    let messages: Vec<interface::Message> = server_state
        .database
        .latest_messages(count as usize)
        .into_iter()
        .map(|message| interface::Message {
            content: message.content.as_ref().to_owned().into(),
            date: message.date,
        })
        .collect();
    let response = FetchMessagesResponse {
        messages: messages.into(),
    };
    let response_json = serde_json::to_string(&response).unwrap();
    println!(
        "Someone fetched {} messages, giving them {} messages",
        fetch_message_form.max_count,
        response.messages.len(),
    );
    Ok(reponse(response_json))
}

async fn read_request_body<T: DeserializeOwned>(request: Incoming) -> DynThreadSafeResult<T> {
    let body = request.collect().await?.aggregate();
    serde_json::from_reader(body.reader()).map_err(Into::into)
}

fn reponse(s: impl Into<Bytes>) -> Response<Full<Bytes>> {
    Response::new(Full::new(s.into()))
}
