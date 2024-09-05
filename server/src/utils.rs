#![allow(dead_code)]

use std::{
    fmt::{self, Debug, Display},
    future::Future,
    marker::Tuple,
    net::SocketAddr,
    sync::Arc,
};

use bytes::{buf::Reader, Buf, Bytes};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, header::HeaderValue, HeaderMap, Request, StatusCode, Version};
use interface::routes::HttpMethod;
use serde::{de::DeserializeOwned, Serialize};
use tokio_tungstenite::tungstenite::http::Extensions;

pub type DynLocalError = Box<dyn std::error::Error>;
pub type DynLocalResult<T> = Result<T, DynLocalError>;
pub type DynError = Box<dyn std::error::Error + Send + Sync>;
pub type DynResult<T> = Result<T, DynError>;

pub trait ServerState: Send + Sync {}
impl<T: Send + Sync> ServerState for T {}

pub fn respond(bytes: impl Into<Bytes>) -> hyper::Response<Full<Bytes>> {
    respond_with_status(StatusCode::OK, bytes)
}

pub fn respond_with_status(
    status: StatusCode,
    bytes: impl Into<Bytes>,
) -> hyper::Response<Full<Bytes>> {
    hyper::Response::builder()
        .status(status)
        .body(Full::new(bytes.into()))
        .unwrap()
}

/// A request in its unextracted state.
#[derive(Debug)]
pub struct UnextractedRequest<S: ServerState> {
    pub server_state: Arc<S>,
    pub remote_addr: SocketAddr,
    pub method: HttpMethod,
    pub uri: hyper::Uri,
    pub http_version: Version,
    pub http_headers: HeaderMap<HeaderValue>,
    pub extensions: Extensions,
    pub body: Option<Incoming>,
    pub path: String,
}

impl<S: ServerState> UnextractedRequest<S> {
    pub fn new(
        server_state: Arc<S>,
        remote_addr: SocketAddr,
        method: HttpMethod,
        path: String,
        request: Request<Incoming>,
    ) -> Self {
        Self {
            server_state,
            remote_addr,
            method,
            uri: request.uri().clone(),
            http_version: request.version(),
            http_headers: request.headers().clone(),
            extensions: request.extensions().clone(),
            body: Some(request.into_body()),
            path,
        }
    }
    pub async fn handle_by<Marker, Handler, Output>(
        &mut self,
        f: Handler,
    ) -> DynResult<Response>
    where
        Marker: Tuple,
        Output: IntoResponse,
        Handler: RequestHandlerFn<S, Marker, Output>,
    {
        f.handle(self).await.map(IntoResponse::into_response)
    }
}

/// Half ass emulation of Axum's extractors.
pub trait Extractor<S: ServerState>: Sized {
    fn extract(
        request: &mut UnextractedRequest<S>,
    ) -> impl Future<Output = DynResult<Self>>;
}

/// Extractor for receiving the server state.
pub struct State<S: ServerState>(pub Arc<S>);
impl<S: ServerState> Extractor<S> for State<S> {
    async fn extract(request: &mut UnextractedRequest<S>) -> DynResult<Self> {
        Ok(Self(request.server_state.clone()))
    }
}

/// Extractor for receiving the remote address.
pub struct RemoteAddr(pub SocketAddr);
impl<S: ServerState> Extractor<S> for RemoteAddr {
    async fn extract(request: &mut UnextractedRequest<S>) -> DynResult<Self> {
        Ok(Self(request.remote_addr))
    }
}

/// Extractor for receiving the body and deserialize it by JSON into a thing.
pub struct Json<T: DeserializeOwned>(pub T);
impl<S: ServerState, T: DeserializeOwned> Extractor<S> for Json<T> {
    async fn extract(request: &mut UnextractedRequest<S>) -> DynResult<Self> {
        // FIXME: Make an error type for this.
        let body = request.body.take().unwrap();
        let reader = body.collect().await?.aggregate().reader();
        let deserialized: T = serde_json::from_reader(reader).unwrap();
        Ok(Self(deserialized))
    }
}

/// Extractor for receiving the path.
pub struct Path(pub String);
impl<S: ServerState> Extractor<S> for Path {
    async fn extract(request: &mut UnextractedRequest<S>) -> DynResult<Self> {
        Ok(Self(request.path.clone()))
    }
}

/// Extractor for receiving the URI.
pub struct Uri(pub hyper::Uri);
impl<S: ServerState> Extractor<S> for Uri {
    async fn extract(request: &mut UnextractedRequest<S>) -> DynResult<Self> {
        Ok(Self(request.uri.clone()))
    }
}

impl<T: DeserializeOwned, B: Buf> TryFrom<Reader<B>> for Json<T> {
    type Error = serde_json::Error;
    fn try_from(value: Reader<B>) -> Result<Self, Self::Error> {
        serde_json::from_reader(value).map(Self)
    }
}

#[derive(Debug, Default)]
pub struct Response {
    inner: hyper::Response<Vec<u8>>,
}

impl Response {
    pub fn status(mut self, new_status: StatusCode) -> Self {
        *self.inner.status_mut() = new_status;
        self
    }
    pub fn into_hyper_response(self) -> hyper::Response<Full<Bytes>> {
        self.inner.map(|bytes| Full::new(bytes.into()))
    }
}

pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Response {
            inner: hyper::Response::new(self),
        }
    }
}

impl IntoResponse for &[u8] {
    fn into_response(self) -> Response {
        Response {
            inner: hyper::Response::new(self.to_vec()),
        }
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        self.into_bytes().into_response()
    }
}

impl IntoResponse for &str {
    fn into_response(self) -> Response {
        self.as_bytes().into_response()
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        Response::default().status(self)
    }
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

#[derive(Clone)]
pub struct Serialized {
    json: String,
}

impl Debug for Serialized {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.json, f)
    }
}

impl Display for Serialized {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.json, f)
    }
}

impl IntoResponse for Serialized {
    fn into_response(self) -> Response {
        self.json.into_response()
    }
}

pub trait ToJson: Serialize {
    fn to_json(&self) -> Serialized {
        Serialized {
            json: serde_json::to_string(self).unwrap(),
        }
    }
}

impl<T: Serialize> ToJson for T {}

pub trait RequestHandlerFn<S: ServerState, Args: Tuple, Output: IntoResponse> {
    fn handle(
        self,
        request: &mut UnextractedRequest<S>,
    ) -> impl Future<Output = DynResult<Output>>;
}

impl<S, F, Fut, Output> RequestHandlerFn<S, (), Output> for F
where
    S: ServerState,
    F: FnOnce() -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, _request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        self().await
    }
}

impl<S, E0, F, Fut, Output> RequestHandlerFn<S, (E0,), Output> for F
where
    S: ServerState,
    E0: Extractor<S>,
    F: FnOnce(E0) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        self(e0).await
    }
}

impl<S, E0, E1, F, Fut, Output> RequestHandlerFn<S, (E0, E1), Output> for F
where
    S: ServerState,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        self(e0, e1).await
    }
}

impl<S, E0, E1, E2, F, Fut, Output> RequestHandlerFn<S, (E0, E1, E2), Output> for F
where
    S: ServerState,
    E2: Extractor<S>,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1, E2) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        let e2 = E2::extract(request).await?;
        self(e0, e1, e2).await
    }
}

impl<S, E0, E1, E2, E3, F, Fut, Output> RequestHandlerFn<S, (E0, E1, E2, E3), Output> for F
where
    S: ServerState,
    E2: Extractor<S>,
    E3: Extractor<S>,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1, E2, E3) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        let e2 = E2::extract(request).await?;
        let e3 = E3::extract(request).await?;
        self(e0, e1, e2, e3).await
    }
}

impl<S, E0, E1, E2, E3, E4, F, Fut, Output> RequestHandlerFn<S, (E0, E1, E2, E3, E4), Output> for F
where
    S: ServerState,
    E2: Extractor<S>,
    E3: Extractor<S>,
    E4: Extractor<S>,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1, E2, E3, E4) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        let e2 = E2::extract(request).await?;
        let e3 = E3::extract(request).await?;
        let e4 = E4::extract(request).await?;
        self(e0, e1, e2, e3, e4).await
    }
}

impl<S, E0, E1, E2, E3, E4, E5, F, Fut, Output>
    RequestHandlerFn<S, (E0, E1, E2, E3, E4, E5), Output> for F
where
    S: ServerState,
    E2: Extractor<S>,
    E3: Extractor<S>,
    E4: Extractor<S>,
    E5: Extractor<S>,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1, E2, E3, E4, E5) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        let e2 = E2::extract(request).await?;
        let e3 = E3::extract(request).await?;
        let e4 = E4::extract(request).await?;
        let e5 = E5::extract(request).await?;
        self(e0, e1, e2, e3, e4, e5).await
    }
}

impl<S, E0, E1, E2, E3, E4, E5, E6, F, Fut, Output>
    RequestHandlerFn<S, (E0, E1, E2, E3, E4, E5, E6), Output> for F
where
    S: ServerState,
    E2: Extractor<S>,
    E3: Extractor<S>,
    E4: Extractor<S>,
    E5: Extractor<S>,
    E6: Extractor<S>,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1, E2, E3, E4, E5, E6) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        let e2 = E2::extract(request).await?;
        let e3 = E3::extract(request).await?;
        let e4 = E4::extract(request).await?;
        let e5 = E5::extract(request).await?;
        let e6 = E6::extract(request).await?;
        self(e0, e1, e2, e3, e4, e5, e6).await
    }
}

impl<S, E0, E1, E2, E3, E4, E5, E6, E7, F, Fut, Output>
    RequestHandlerFn<S, (E0, E1, E2, E3, E4, E5, E6, E7), Output> for F
where
    S: ServerState,
    E2: Extractor<S>,
    E3: Extractor<S>,
    E4: Extractor<S>,
    E5: Extractor<S>,
    E6: Extractor<S>,
    E7: Extractor<S>,
    E1: Extractor<S>,
    E0: Extractor<S>,
    F: FnOnce(E0, E1, E2, E3, E4, E5, E6, E7) -> Fut,
    Fut: Future<Output = DynResult<Output>>,
    Output: IntoResponse,
{
    async fn handle(self, request: &mut UnextractedRequest<S>) -> DynResult<Output> {
        let e0 = E0::extract(request).await?;
        let e1 = E1::extract(request).await?;
        let e2 = E2::extract(request).await?;
        let e3 = E3::extract(request).await?;
        let e4 = E4::extract(request).await?;
        let e5 = E5::extract(request).await?;
        let e6 = E6::extract(request).await?;
        let e7 = E7::extract(request).await?;
        self(e0, e1, e2, e3, e4, e5, e6, e7).await
    }
}
