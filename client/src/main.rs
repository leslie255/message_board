#![feature(iter_collect_into)]

mod api;
mod state;
mod utils;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use flexi_logger::{FileSpec, Logger, WriteMode};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use state::AppState;
use std::env;
use std::{io, sync::Arc};
use tokio::time::{self, Duration};
use utils::PrettyUnwrap;

pub const DISPLAY_MESSAGE_COUNT: usize = 20;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let _logger = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default())
        .write_mode(WriteMode::BufferAndFlush)
        .start()
        .unwrap();

    let server_url = env::args().nth(1).unwrap_or("http://127.0.0.1:3000".into());

    // Create app state
    let app_state = AppState::with_server(server_url);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Spawn an async task to generate fake messages
    {
        let app_state = Arc::clone(&app_state);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                app_state
                    .fetch_new_messages_if_needed()
                    .await
                    .pretty_unwrap();
            }
        });
    };

    'event_loop: loop {
        terminal.draw(|f| {
            draw_ui(f, &app_state);
        })?;

        if !crossterm::event::poll(Duration::from_millis(100))? {
            continue 'event_loop;
        }
        let Event::Key(key) = event::read()? else {
            continue 'event_loop;
        };
        match key.code {
            KeyCode::Char(c) => {
                app_state.lock_input().push(c);
            }
            KeyCode::Backspace => {
                app_state.lock_input().pop();
            }
            KeyCode::Enter => {
                app_state.send_message().await.unwrap();
            }
            KeyCode::Esc => {
                break 'event_loop;
            }
            _ => {}
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_ui(f: &mut ratatui::Frame, app_state: &AppState) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(size);

    let messages: Vec<ListItem> = app_state
        .lock_messages()
        .iter()
        .map(|message| ListItem::new(message.content.to_string()))
        .collect();
    let messages_list =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages"));
    f.render_widget(messages_list, chunks[0]);

    // Input field
    let input = Paragraph::new(app_state.lock_input().to_string())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);
}
