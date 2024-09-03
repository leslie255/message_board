#![allow(dead_code)]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
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
    pub fn purge_before(&self, before_date: DateTime<Utc>) {
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

    pub fn purge_6_hours_ago(&self) {
        let six_hours_ago = Utc::now() - Duration::hours(6);
        self.purge_before(six_hours_ago);
    }

    fn messages(&self) -> MutexGuard<VecDeque<Message>> {
        self.messages.lock().unwrap()
    }

    pub fn add_message(&self, message: Message) {
        let is_invisible =
            message.content.is_empty() || !message.content.chars().any(|c| !c.is_whitespace());
        if !is_invisible {
            self.messages().push_back(message);
        }
    }

    pub fn message_count(&self) -> usize {
        self.messages().len()
    }

    pub fn for_each_message(&self, mut f: impl FnMut(&Message)) {
        for message in self.messages().iter() {
            f(message);
        }
    }

    pub fn latest_messages(&self, count: usize) -> Vec<Message> {
        self.messages().iter().rev().take(count).cloned().collect()
    }

    /// Returns `None` if there are no messages.
    pub fn latest_message_date(&self) -> Option<DateTime<Utc>> {
        self.messages().back().map(|message| message.date)
    }
}
