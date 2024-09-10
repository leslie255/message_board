#![feature(decl_macro, tuple_trait, never_type)]

/// Emulates a data base, will swap out with a real one later.
mod database;

mod utils;

/// Manages everything Websocket.
mod websocket;

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing, Json, Router};
use database::DataBase;
use interface::{
    FetchLatestUpdateDateForm, FetchLatestUpdateDateResponse, FetchMessagesForm,
    FetchMessagesResponse, SendMessageForm, SendMessageResponse,
};

use crate::{database::Message, utils::DynResult};

#[allow(unused_imports)]
use crate::utils::todo_;

#[derive(Clone, Default)]
struct ServerState {
    database: Arc<DataBase>,
}

#[tokio::main]
pub async fn main() -> DynResult<()> {
    let server_state = ServerState::default();
    let app = Router::new()
        .route("/hello", routing::get(hello))
        .route("/send_message", routing::post(send_message))
        .route("/fetch_messages", routing::get(fetch_messages))
        .route(
            "/fetch_latest_update_date",
            routing::get(fetch_latest_update_date),
        )
        .with_state(server_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn hello() -> impl IntoResponse {
    "HELLO, WORLD"
}

async fn send_message(
    State(server_state): State<ServerState>,
    Json(form): Json<SendMessageForm>,
) -> impl IntoResponse {
    let message = Message::new(form.content.into());
    server_state.database.add_message(message);
    Json(SendMessageResponse::ok())
}

async fn fetch_messages(
    State(server_state): State<ServerState>,
    Json(form): Json<FetchMessagesForm>,
) -> impl IntoResponse {
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
            id: message.id,
            content: message.content.as_ref().to_owned().into(),
            date: message.date,
        })
        .collect();
    log::info!(
        "Responding fetch messages request with {} messages",
        messages.len()
    );
    Json(FetchMessagesResponse {
        messages: messages.into(),
    })
}

async fn fetch_latest_update_date(
    State(server_state): State<ServerState>,
    Json(_): Json<FetchLatestUpdateDateForm>,
) -> impl IntoResponse {
    Json(FetchLatestUpdateDateResponse {
        latest_update_date: server_state.database.latest_message_date(),
    })
}
