#![allow(dead_code)]

use bytes::{Buf, Bytes};
use chrono::{DateTime, Utc};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, Method, Request, Response, Uri};
use hyper_util::rt::TokioIo;
use interface::{
    routes, FetchLatestUpdateDateForm, FetchLatestUpdateDateResponse, FetchMessagesForm,
    FetchMessagesResponse, HttpMethod, Message, SendMessageForm, SendMessageResponse,
};
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

    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    async fn request<T: Serialize, U: DeserializeOwned>(
        &self,
        (method, path): (HttpMethod, &str),
        body: T,
    ) -> DynThreadSafeResult<U> {
        let uri: Uri = format!("{}{path}", &self.server_url).parse().unwrap();
        let method: hyper::Method = method.try_into()?;
        let response: U = request(uri, method, Some(body)).await?;
        Ok(response)
    }

    pub async fn test_connection(&self) -> bool {
        self.test_connection_()
            .await
            .is_ok_and(std::convert::identity)
    }

    /// Helper function for `test_connection` until rust stablizes try blocks.
    async fn test_connection_(&self) -> DynThreadSafeResult<bool> {
        // Unfortunately this is much of a rewrite of `Self::request` due to response to GET /hello
        // not being JSON.
        let (method, path) = routes::HELLO;
        let method: hyper::Method = method.try_into()?;
        let uri: Uri = format!("{}{path}", &self.server_url).parse().unwrap();
        let response = request_raw(uri, method, None::<()>).await?;
        let response_string = collect_response_to_string(response).await?;
        Ok(response_string.as_str() == interface::EXPECTED_RESPONSE_TO_HELLO)
    }

    pub async fn send_message(&self, content: Box<str>) -> DynThreadSafeResult<()> {
        let response: SendMessageResponse = self
            .request(routes::SEND_MESSAGE, SendMessageForm { content })
            .await?;
        assert!(response.ok);
        Ok(())
    }

    pub async fn fetch_messages(
        &self,
        max_count: u32,
        since: Option<DateTime<Utc>>,
    ) -> DynThreadSafeResult<Box<[Message]>> {
        let response: FetchMessagesResponse = self
            .request(
                routes::FETCH_MESSAGES,
                FetchMessagesForm { max_count, since },
            )
            .await?;
        Ok(response.messages)
    }

    pub async fn fetch_latest_update_date(&self) -> DynThreadSafeResult<Option<DateTime<Utc>>> {
        let response: FetchLatestUpdateDateResponse = self
            .request(
                routes::FETCH_LATEST_UPDATE_DATE,
                FetchLatestUpdateDateForm {},
            )
            .await?;
        Ok(response.latest_update_date)
    }
}

async fn request_raw(
    url: Uri,
    method: Method,
    body: Option<impl Serialize>,
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
    let body_string = match body {
        Some(ref body) => serde_json::to_string(body)?,
        None => String::new(),
    };
    let path = url.path();
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(hyper::header::HOST, authority.as_str())
        .body(Full::new(Bytes::from(body_string)))?;
    let response = sender.send_request(request).await?;
    Ok(response)
}

async fn collect_response_to_string(response: Response<Incoming>) -> DynThreadSafeResult<String> {
    let response_body = response.collect().await?.to_bytes();
    let response_string = String::from_utf8(response_body.to_vec())?;
    Ok(response_string)
}

async fn request<T: DeserializeOwned>(
    url: Uri,
    method: Method,
    body: Option<impl Serialize>,
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
    let response_body = request_raw(url, method, Some(body)).await?;
    let response_string = collect_response_to_string(response_body).await?;
    let x = serde_json::from_str(&response_string)?;
    Ok((x, response_string))
}
