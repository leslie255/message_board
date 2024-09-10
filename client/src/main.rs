#![feature(iter_collect_into, new_range_api, decl_macro)]

mod api;
mod state;
mod tui;
mod utils;
mod input_field;

use flexi_logger::{FileSpec, Logger, WriteMode};
use state::AppState;
use std::{env, sync::Arc};
use utils::DynResult;

#[tokio::main]
async fn main() -> DynResult<()> {
    let _logger = Logger::try_with_str("info")?
        .log_to_file(FileSpec::default())
        .write_mode(WriteMode::BufferAndFlush)
        .start()?;

    let server_url = env::args().nth(1).unwrap_or("http://127.0.0.1:3000".into());
    let app_state = AppState::with_server(server_url);

    app_state.fetch_new_messages_if_needed().await?;

    println!("Saying hello with server");
    log::info!("Saying hello with server");
    if !app_state.api().test_connection().await {
        println!("Can't connect with server {}", app_state.api().server_url());
        log::error!("Can't connect with server {}", app_state.api().server_url());
        std::process::exit(1);
    }

    state::setup_background_update(Arc::clone(&app_state));

    let mut terminal = tui::setup_terminal();
    tui::event_loop(&mut terminal, Arc::clone(&app_state))?;
    tui::restore_terminal();

    Ok(())
}
