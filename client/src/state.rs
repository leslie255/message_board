use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use chrono::{DateTime, Utc};
use interface::Message;
use tokio::time;

use crate::{
    api,
    newtui::UIState,
    utils::{DynResult, PrettyUnwrap},
};

#[derive(Debug)]
pub struct AppState {
    api: api::Client,
    messages: Mutex<VecDeque<Message>>,
    start_date: DateTime<Utc>,
    ui_state: Mutex<UIState>,
    is_fetching_message: AtomicBool,
}

impl AppState {
    pub fn api(&self) -> &api::Client {
        &self.api
    }

    pub fn with_server(server_url: String) -> Arc<Self> {
        let self_ = Arc::new(Self {
            api: api::Client::with_server(server_url),
            messages: Mutex::new(VecDeque::new()),
            start_date: Utc::now(),
            ui_state: Mutex::new(UIState::default()),
            is_fetching_message: false.into(),
        });
        self_
            .ui_state
            .lock()
            .unwrap()
            .set_app_state(Arc::downgrade(&self_));
        self_
    }

    pub fn lock_messages(&self) -> MutexGuard<VecDeque<Message>> {
        self.messages.lock().pretty_unwrap()
    }

    pub fn lock_ui_state(&self) -> MutexGuard<UIState> {
        self.ui_state.lock().pretty_unwrap()
    }

    pub async fn fetch_new_messages_if_needed(&self) -> DynResult<()> {
        if self.is_fetching_message() {
            return Ok(());
        }
        self.set_is_fetching_message();
        let local_latest = self.lock_messages().back().map(|message| message.date);
        let remote_latest = self.api.fetch_latest_update_date().await?;
        let need_update = match (local_latest, remote_latest) {
            (Some(local), Some(remote)) => remote >= local,
            (None, None) => false,
            _ => true,
        };
        log::debug!(
            "local: {local_latest:?}, remote: {remote_latest:?}, need_update: {need_update}"
        );
        if need_update {
            let new_messages = self.api.fetch_messages(100, local_latest).await?;
            let mut messages = self.lock_messages();
            let messages: &mut VecDeque<Message> = &mut messages;
            // To pervent latest message being repeated.
            if let Some(local_latest) = local_latest {
                // FIXME: Optimize this with assumption of message being ordered chronologically.
                messages.retain(|message| message.date != local_latest);
            }
            new_messages.into_vec().into_iter().collect_into(messages);
        }
        self.unset_is_fetching_message();
        Ok(())
    }

    pub fn start_date(&self) -> DateTime<Utc> {
        self.start_date
    }

    fn is_fetching_message(&self) -> bool {
        self.is_fetching_message.load(Ordering::Acquire)
    }

    fn set_is_fetching_message(&self) {
        self.is_fetching_message.store(true, Ordering::Release);
    }

    fn unset_is_fetching_message(&self) {
        self.is_fetching_message.store(false, Ordering::Release);
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
