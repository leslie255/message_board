use std::{io::Stdout, sync::Arc};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use interface::Message;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

use crate::{state::AppState, utils::DynResult};

pub fn setup_terminal() -> Terminal<CrosstermBackend<Stdout>> {
    ratatui::init()
}

pub fn restore_terminal() {
    ratatui::restore()
}

pub async fn event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app_state: Arc<AppState>,
) -> DynResult<()> {
    'event_loop: loop {
        terminal.draw(|frame| {
            draw(frame, &app_state);
        })?;
        if !crossterm::event::poll(std::time::Duration::from_millis(100))? {
            continue 'event_loop;
        }
        let Event::Key(key) = event::read()? else {
            continue 'event_loop;
        };
        match key.code {
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app_state
                    .fetch_new_messages_if_needed()
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Error fetching messages: {e}");
                    });
            }
            KeyCode::Char(char) => {
                app_state.lock_input().push(char);
            }
            KeyCode::Backspace => {
                app_state.lock_input().pop();
            }
            KeyCode::Enter => {
                app_state.send_message().await.unwrap_or_else(|e| {
                    log::error!("Error sending message: {e}");
                });
            }
            KeyCode::Esc => {
                break 'event_loop Ok(());
            }
            _ => {}
        }
    }
}

pub fn format_message(message: &Message) -> String {
    format!(
        "[{}] {}",
        message.date.format("%Y-%m-%d %H:%M:%S"),
        message.content
    )
}

pub fn draw(frame: &mut ratatui::Frame, app_state: &AppState) {
    let size = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(size);

    let messages: Vec<ListItem> = app_state
        .lock_messages()
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|message| ListItem::new(format_message(message)))
        .collect();
    let messages_list =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages"));
    frame.render_widget(messages_list, chunks[0]);

    // Input field
    let input = Paragraph::new(app_state.lock_input().to_string())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    frame.render_widget(input, chunks[1]);
}
