use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

use interface::Message;
use tokio::time;

use crate::{
    api,
    tui::UIState,
    utils::{DynResult, PrettyUnwrap},
    DISPLAY_MESSAGE_COUNT,
};

#[derive(Debug)]
pub struct AppState {
    api: api::Client,
    messages: Mutex<VecDeque<Message>>,
    /// States related to UI elements.
    ui_state: Mutex<UIState>,
}

impl AppState {
    pub fn api(&self) -> &api::Client {
        &self.api
    }

    pub fn with_server(server_url: String) -> Arc<Self> {
        Arc::new(Self {
            api: api::Client::with_server(server_url),
            messages: Mutex::new(VecDeque::new()),
            ui_state: Mutex::new(UIState::default()),
        })
    }

    pub fn lock_messages(&self) -> MutexGuard<VecDeque<Message>> {
        self.messages.lock().pretty_unwrap()
    }

    pub fn lock_ui_state(&self) -> MutexGuard<UIState> {
        self.ui_state.lock().pretty_unwrap()
    }

    pub async fn fetch_new_messages_if_needed(&self) -> DynResult<()> {
        let local_latest = self.lock_messages().back().map(|message| message.date);
        let remote_latest = self.api.fetch_latest_update_date().await?;
        let need_update = match (local_latest, remote_latest) {
            (Some(local), Some(remote)) => remote >= local,
            (None, None) => false,
            _ => true,
        };
        log::info!(
            "local: {local_latest:?}, remote: {remote_latest:?}, need_update: {need_update}"
        );
        if need_update {
            let new_messages = self
                .api
                .fetch_messages(DISPLAY_MESSAGE_COUNT as u32, local_latest)
                .await?;
            let mut messages = self.lock_messages();
            let messages: &mut VecDeque<Message> = &mut messages;
            // To pervent latest message being repeated.
            if let Some(local_latest) = local_latest {
                // FIXME: Optimize this with assumption of message being ordered chronologically.
                messages.retain(|message| message.date != local_latest);
            }
            new_messages.into_vec().into_iter().collect_into(messages);
        }
        Ok(())
    }

    pub async fn send_message(&self) -> DynResult<()> {
        let new_message: Box<str> = {
            let mut ui_state = self.lock_ui_state();
            std::mem::take(ui_state.input_mut()).into()
        };
        self.api.send_message(new_message).await?;
        self.fetch_new_messages_if_needed().await?;
        Ok(())
    }
}

pub fn setup_background_update(app_state: Arc<AppState>) {
    let app_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            app_state
                .fetch_new_messages_if_needed()
                .await
                .pretty_unwrap();
        }
    });
}
