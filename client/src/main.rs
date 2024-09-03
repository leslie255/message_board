#![feature(iter_collect_into)]

mod api;

pub type DynError = Box<dyn std::error::Error>;
pub type DynThreadSafeError = Box<dyn std::error::Error + Send + Sync>;
pub type DynResult<T> = Result<T, DynError>;
pub type DynThreadSafeResult<T> = Result<T, DynThreadSafeError>;

use api::Client;
use cursive::event::{Event, EventResult, Key};
use cursive::traits::*;
use cursive::view::ViewWrapper;
use cursive::views::{
    Dialog, DummyView, EditView, LinearLayout, ListChild, ListView, NamedView, ResizedView,
    TextView,
};
use cursive::Cursive;
use flexi_logger::{FileSpec, Logger, WriteMode};
use interface::Message;

const DISPLAY_MESSAGE_COUNT: usize = 20;

const MESSAGES_LIST_VIEW_NAME: &str = "message_list";
const MESSAGE_EDIT_VIEW_NAME: &str = "message_edit_view";

struct State {
    api_client: Client,
}

/// Fetch new messages and update message list with it.
fn refresh_message_list(siv: &mut Cursive) {
    log::info!("[{}:{}] here", file!(), line!());
    let new_messages = {
        let api_client = &mut siv.user_data::<State>().unwrap().api_client;
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(api_client.fetch_messages(DISPLAY_MESSAGE_COUNT as u32))
            .unwrap()
    };
    log::info!("[{}:{}] here", file!(), line!());
    update_message_list_with(siv, &new_messages);
    log::info!("[{}:{}] here", file!(), line!());
}

fn update_message_list_with(siv: &mut Cursive, messages: &[Message]) {
    log::info!("[{}:{}] here", file!(), line!());
    log::info!("[{}:{}] here", file!(), line!());
    let mut message_list = siv.find_name::<ListView>(MESSAGES_LIST_VIEW_NAME).unwrap();
    log::info!("[{}:{}] here", file!(), line!());
    let mut new_children: Vec<ListChild> = Vec::with_capacity(DISPLAY_MESSAGE_COUNT);
    (messages.len()..DISPLAY_MESSAGE_COUNT)
        .map(|_| ListChild::Row(String::new(), Box::new(DummyView::new())))
        .collect_into(&mut new_children);
    messages
        .iter()
        .rev()
        .map(|Message { content, date }| {
            let text_view = TextView::new(format!("[{date}] {content}"));
            ListChild::Row(String::new(), Box::new(text_view))
        })
        .collect_into(&mut new_children);
    log::info!("[{}:{}] here", file!(), line!());
    message_list.set_children(new_children);
    log::info!("[{}:{}] here", file!(), line!());
}

/// Send text as message, clears the editor.
fn send_message(siv: &mut Cursive, text: &str) {
    siv.call_on_name(MESSAGE_EDIT_VIEW_NAME, |view: &mut EditView| {
        view.set_content(""); // Clear the input field after sending.
    });
    let api_client = &mut siv.user_data::<State>().unwrap().api_client;
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(api_client.send_message(text.into())).unwrap();
    refresh_message_list(siv);
}

struct MessageEditView {
    inner: ResizedView<NamedView<EditView>>,
}

impl MessageEditView {
    fn new() -> Self {
        MessageEditView {
            inner: EditView::new()
                .on_submit(send_message)
                .with_name(MESSAGE_EDIT_VIEW_NAME)
                .fixed_width(50),
        }
    }
}

impl ViewWrapper for MessageEditView {
    cursive::wrap_impl! { self.inner: ResizedView<NamedView<EditView>> }

    fn wrap_on_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Key(Key::Esc) => EventResult::with_cb(|siv| {
                siv.focus_name("message_list").unwrap();
            }),
            event => self.inner.on_event(event),
        }
    }
}

struct MessageListView {
    inner: NamedView<ListView>,
}

impl MessageListView {
    fn new() -> Self {
        let list_view = {
            let mut list = ListView::new();
            for _ in 0..DISPLAY_MESSAGE_COUNT {
                list.add_child("", TextView::new(" "));
            }
            list.with_name(MESSAGES_LIST_VIEW_NAME)
        };
        Self { inner: list_view }
    }
}

impl ViewWrapper for MessageListView {
    cursive::wrap_impl! { self.inner: NamedView<ListView> }

    fn wrap_on_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Char('r') => EventResult::with_cb(|siv| {
                refresh_message_list(siv);
            }),
            event => self.inner.on_event(event),
        }
    }
}

fn main() {
    let _logger = Logger::try_with_str("info, my::critical::module=trace")
        .unwrap()
        .log_to_file(FileSpec::default())
        .write_mode(WriteMode::BufferAndFlush)
        .start()
        .unwrap();

    let state = State {
        api_client: Client::with_server("http://127.0.0.1:3000".into()),
    };

    let mut siv = cursive::default();
    siv.set_user_data(state);

    let message_list = MessageListView::new();

    let layout = LinearLayout::vertical()
        .child(Dialog::around(message_list).title("Messages"))
        .child(MessageEditView::new());

    siv.add_layer(layout);

    refresh_message_list(&mut siv);

    siv.run();
}
