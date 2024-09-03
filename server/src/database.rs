#![allow(dead_code)]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone)]
pub struct Message {
    pub content: Arc<str>,
    pub date: DateTime<Utc>,
}

impl Message {
    pub fn now(content: Arc<str>) -> Self {
        Self {
            content,
            date: Utc::now(),
        }
    }
}

#[derive(Debug, Default)]
pub struct DataBase {
    /// Messages are ordered by date.
    messages: Mutex<VecDeque<Message>>,
}

fn vec_deque_remove_before<T>(vec: &mut VecDeque<T>, idx: usize) {
    // FIXME: Unneeded allocation.
    let after = vec.split_off(idx);
    *vec = after;
}

impl DataBase {
    /// Delete all messages before a date.
    pub fn purge_before(&mut self, before_date: DateTime<Utc>) {
        let mut messages = self.messages.lock().unwrap();
        if messages.front().is_some_and(|x| x.date < before_date) {
            return;
        }
        if let Some(idx) = messages
            .iter()
            .position(|message| message.date > before_date)
        {
            vec_deque_remove_before(&mut messages, idx);
        };
    }

    pub fn purge_6_hours_ago(&mut self) {
        let six_hours_ago = Utc::now() - Duration::hours(6);
        self.purge_before(six_hours_ago);
    }

    pub fn add_message(&self, message: Message) {
        self.messages.lock().unwrap().push_front(message);
    }

    pub fn message_count(&self) -> usize {
        self.messages.lock().unwrap().len()
    }

    pub fn for_each_message(&self, mut f: impl FnMut(&Message)) {
        for message in self.messages.lock().unwrap().iter() {
            f(message);
        }
    }

    pub fn latest_messages(&self, count: usize) -> Vec<Message> {
        self
            .messages
            .lock()
            .unwrap()
            .iter()
            .take(count)
            .cloned()
            .collect()
    }
}
