#![feature(iter_collect_into, new_range_api, decl_macro)]

mod api;
mod input_field;
mod newtui;
mod state;
mod utils;

use flexi_logger::{FileSpec, Logger, WriteMode};
use state::AppState;
use std::{env, sync::Arc};
use utils::DynResult;

const DEFAULT_SERVER_URL: &str = if cfg!(debug_assertions) {
    "http://127.0.0.1:3000"
} else {
    "http://64.176.51.97:3000"
};

#[tokio::main]
async fn main() -> DynResult<()> {
    let _logger = Logger::try_with_str("info")?
        .log_to_file(FileSpec::default())
        .write_mode(WriteMode::BufferAndFlush)
        .start()?;

    let server_url = env::args().nth(1).unwrap_or(DEFAULT_SERVER_URL.into());
    let app_state = AppState::with_server(server_url);

    println!("Saying hello with server");
    log::info!("Saying hello with server");
    if !app_state.api().test_connection().await {
        println!("Can't connect with server {}", app_state.api().server_url());
        log::error!("Can't connect with server {}", app_state.api().server_url());
        std::process::exit(1);
    }

    app_state.fetch_new_messages_if_needed().await?;

    state::setup_background_update(Arc::clone(&app_state));

    let mut terminal = domtui::setup_terminal();
    newtui::event_loop(&mut terminal, Arc::clone(&app_state))?;
    domtui::restore_terminal(terminal);

    Ok(())
}
