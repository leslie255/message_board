use std::env;

use api::Client;
use chrono::{DateTime, Local};
use interface::Message;

mod api;

pub type DynError = Box<dyn std::error::Error>;
pub type DynThreadSafeError = Box<dyn std::error::Error + Send + Sync>;
pub type DynResult<T> = Result<T, DynError>;
pub type DynThreadSafeResult<T> = Result<T, DynThreadSafeError>;

#[tokio::main]
async fn main() {
    let arg1 = {
        let mut args = env::args();
        args.nth(1)
    };
    let client = Client::default();
    if let Some(message_to_send) = arg1 {
        client
            .send_message(message_to_send.clone().into())
            .await
            .unwrap();
        println!("Sent message: {message_to_send:?}");
    }
    let messages = client.fetch_messages().await.unwrap();
    for Message { date, content } in messages {
        let local_date: DateTime<Local> = date.into();
        println!("[{local_date}] {content}");
    }
}
