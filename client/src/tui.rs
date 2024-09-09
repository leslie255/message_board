use std::{io::Stdout, sync::Arc};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use interface::Message;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{self, Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

use crate::{state::AppState, utils::DynResult};

/// State of UI elements.
#[derive(Debug, Clone, Default)]
pub struct UIState {
    focused: FocusedElement,
    input: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    MessageList,
    InputField,
}

impl Default for FocusedElement {
    fn default() -> Self {
        Self::InputField
    }
}

impl UIState {
    pub fn focus_next(&mut self) {
        self.focused = match self.focused {
            FocusedElement::MessageList => FocusedElement::InputField,
            FocusedElement::InputField => FocusedElement::MessageList,
        };
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut String {
        &mut self.input
    }
}

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
        let focused = app_state.lock_ui_state().focused;
        match (key.modifiers, key.code, focused) {
            (KeyModifiers::NONE, KeyCode::Tab, _) => {
                app_state.lock_ui_state().focus_next();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('r'), _) => {
                app_state
                    .fetch_new_messages_if_needed()
                    .await
                    .unwrap_or_else(|e| log::error!("Error fetching messages: {e}"));
            }
            (KeyModifiers::CONTROL, KeyCode::Char('q'), _) => {
                break 'event_loop Ok(());
            }
            (KeyModifiers::NONE, KeyCode::Char(char), FocusedElement::InputField) => {
                app_state.lock_ui_state().input_mut().push(char);
            }
            (KeyModifiers::NONE, KeyCode::Backspace, FocusedElement::InputField) => {
                app_state.lock_ui_state().input_mut().pop();
            }
            (KeyModifiers::NONE, KeyCode::Enter, FocusedElement::InputField) => {
                app_state
                    .send_message()
                    .await
                    .unwrap_or_else(|e| log::error!("Error sending message: {e}"));
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

fn draw(frame: &mut ratatui::Frame, app_state: &AppState) {
    let ui_state = app_state.lock_ui_state();

    let size = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(size);

    {
        // Messages list.
        let messages: Vec<ListItem> = app_state
            .lock_messages()
            .iter()
            .rev()
            .take(20)
            .rev()
            .map(|message| ListItem::new(format_message(message)))
            .collect();
        let border_color = match ui_state.focused {
            FocusedElement::MessageList => focused_border_color(),
            _ => unfocused_border_color(),
        };
        let messages_list = List::new(messages).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(border_color))
                .title(format!("Server: {}", app_state.api().server_url()))
                .title_style(title_style()),
        );
        frame.render_widget(messages_list, chunks[0]);
    }

    {
        // Input field.
        let text = ui_state.input.clone();
        let border_color = match ui_state.focused {
            FocusedElement::InputField => focused_border_color(),
            _ => unfocused_border_color(),
        };
        let input_field = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::all())
                    .border_style(Style::new().fg(border_color))
                    .title_bottom(return_to_send()),
            )
            .wrap(ratatui::widgets::Wrap { trim: true });
        frame.render_widget(input_field, chunks[1]);
    }
}

fn title_style() -> Style {
    Style::new().add_modifier(style::Modifier::BOLD)
}

fn return_to_send() -> &'static str {
    if cfg!(target_os = "macos") {
        "<RETURN> to send"
    } else {
        "<ENTER> to send"
    }
}

fn focused_border_color() -> Color {
    Color::Yellow
}

fn unfocused_border_color() -> Color {
    Color::White
}
