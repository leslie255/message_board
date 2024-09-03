#![allow(dead_code)]

use bytes::{Buf, Bytes};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, Method, Request, Response, Uri};
use hyper_util::rt::TokioIo;
use interface::{FetchMessagesResponse, Message, SendMessageForm, SendMessageResponse};
use serde::{de::DeserializeOwned, Serialize};
use tokio::net::TcpStream;

use crate::DynThreadSafeResult;

#[derive(Debug, Clone)]
pub struct Client {
    server_url: String,
}

impl Default for Client {
    fn default() -> Self {
        Self::with_server(String::from("http://127.0.0.1:3000"))
    }
}

impl Client {
    pub fn with_server(mut server_url: String) -> Self {
        if server_url.chars().next_back().is_some_and(|c| c == '/') {
            server_url.pop().unwrap();
        }
        Self { server_url }
    }

    pub async fn send_message(&self, content: Box<str>) -> DynThreadSafeResult<()> {
        let url: Uri = format!("{}/send_message", &self.server_url)
            .parse()
            .unwrap();
        let send_message_form = SendMessageForm { content };
        let response: SendMessageResponse = request(url, Method::POST, send_message_form).await?;
        assert!(response.ok);
        Ok(())
    }

    pub async fn fetch_messages(&self, max_count: u32) -> DynThreadSafeResult<Box<[Message]>> {
        let url: Uri = format!("{}/fetch_messages", &self.server_url)
            .parse()
            .unwrap();
        let fetch_message_form = interface::FetchMessagesForm { max_count };
        let response: FetchMessagesResponse = request(url, Method::GET, fetch_message_form).await?;
        Ok(response.messages)
    }
}

async fn request_raw(
    url: Uri,
    method: Method,
    body: impl Serialize,
) -> DynThreadSafeResult<Response<Incoming>> {
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(addr).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });
    let authority = url.authority().unwrap().clone();
    let body_string = serde_json::to_string(&body)?;
    let path = url.path();
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(hyper::header::HOST, authority.as_str())
        .body(Full::new(Bytes::from(body_string)))?;
    let response = sender.send_request(request).await?;
    Ok(response)
}

async fn request<T: DeserializeOwned>(
    url: Uri,
    method: Method,
    body: impl Serialize,
) -> DynThreadSafeResult<T> {
    let response_body = request_raw(url, method, body)
        .await?
        .collect()
        .await?
        .aggregate();
    serde_json::from_reader(response_body.reader()).map_err(Into::into)
}

/// Like `request`, but also get the response as string.
async fn request_and_get_string<T: DeserializeOwned>(
    url: Uri,
    method: Method,
    body: impl Serialize,
) -> DynThreadSafeResult<(T, String)> {
    let response_body = request_raw(url, method, body)
        .await?
        .collect()
        .await?
        .to_bytes();
    let response_string = String::from_utf8(response_body.to_vec()).unwrap();
    let x = serde_json::from_reader(response_body.reader())?;
    Ok((x, response_string))
}
