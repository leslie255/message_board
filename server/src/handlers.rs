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
    Ok(FetchLatestUpdateDateResponse {
        latest_update_date: server_state.database.latest_message_date(),
    }
    .to_json())
}
