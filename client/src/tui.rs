use std::{io::Stdout, sync::Arc};

use chrono::{Timelike, Utc};
use interface::Message;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        self,
        event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::{self, Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};

use crate::{
    input_field::{Cursor, InputFieldState},
    state::AppState,
    utils::DynResult,
};

/// State of UI elements.
#[derive(Debug, Clone, Default)]
pub struct UIState {
    focused: FocusedElement,
    input_field_state: InputFieldState,
    is_in_help_page: bool,
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

    pub fn input_field_state(&self) -> &InputFieldState {
        &self.input_field_state
    }

    pub fn input_field_state_mut(&mut self) -> &mut InputFieldState {
        &mut self.input_field_state
    }
}

pub fn setup_terminal() -> Terminal<CrosstermBackend<Stdout>> {
    ratatui::init()
}

pub fn restore_terminal() {
    ratatui::restore()
}

pub fn event_loop<B: Backend>(
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
        match global_handle_key(&app_state, key) {
            GlobalHandleKeyResult::Continue => continue 'event_loop,
            GlobalHandleKeyResult::Break => break 'event_loop Ok(()),
            GlobalHandleKeyResult::Pass => (),
        }
        let focused_element = app_state.lock_ui_state().focused;
        match focused_element {
            FocusedElement::InputField => input_field_handle_key(&app_state, key),
            FocusedElement::MessageList => message_list_handle_key(&app_state, key),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GlobalHandleKeyResult {
    Continue,
    Break,
    /// Pass the handling of key event onto the currently focused UI element.
    Pass,
}

pub fn global_handle_key(app_state: &AppState, key: KeyEvent) -> GlobalHandleKeyResult {
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('h')) => {
            let mut ui_state = app_state.lock_ui_state();
            ui_state.is_in_help_page = true;
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::NONE, KeyCode::Esc) => {
            let mut ui_state = app_state.lock_ui_state();
            ui_state.is_in_help_page = false;
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::NONE, KeyCode::Tab) => {
            app_state.lock_ui_state().focus_next();
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => GlobalHandleKeyResult::Break,
        _ => GlobalHandleKeyResult::Pass,
    }
}

pub fn input_field_handle_key(app_state: &Arc<AppState>, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char(char)) => {
            app_state
                .lock_ui_state()
                .input_field_state_mut()
                .insert(char);
        }
        (KeyModifiers::NONE, KeyCode::Left) => {
            app_state
                .lock_ui_state()
                .input_field_state_mut()
                .caret_left();
        }
        (KeyModifiers::NONE, KeyCode::Right) => {
            app_state
                .lock_ui_state()
                .input_field_state_mut()
                .caret_right();
        }
        (KeyModifiers::SHIFT, KeyCode::Char(char)) => {
            // FIXME: Respect more advanced keyboard layout (such as those with AltGr).
            let mut ui_state = app_state.lock_ui_state();
            for char in char.to_uppercase() {
                ui_state.input_field_state_mut().insert(char);
            }
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            app_state
                .lock_ui_state()
                .input_field_state_mut()
                .delete_backward();
        }
        (KeyModifiers::NONE, KeyCode::Enter) => {
            let app_state = Arc::clone(app_state);
            tokio::spawn(async move {
                app_state
                    .send_message()
                    .await
                    .unwrap_or_else(|e| log::error!("Error sending message: {e}"))
            });
        }
        _ => (),
    }
}

pub fn message_list_handle_key(app_state: &Arc<AppState>, key: KeyEvent) {
    #[allow(clippy::single_match)] // stfu clippy
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('r')) => {
            let app_state = Arc::clone(app_state);
            tokio::spawn(async move {
                app_state
                    .fetch_new_messages_if_needed()
                    .await
                    .unwrap_or_else(|e| log::error!("Error fetching messages: {e}"));
            });
        }
        _ => (),
    }
}

fn format_message(message: &Message) -> String {
    format!(
        "[{}] {}",
        message.date.format("%Y-%m-%d %H:%M:%S"),
        message.content
    )
}

fn draw(frame: &mut ratatui::Frame, app_state: &AppState) {
    if app_state.lock_ui_state().is_in_help_page {
        draw_help_page(frame)
    } else {
        draw_main_page(frame, app_state)
    }
}

fn draw_help_page(frame: &mut ratatui::Frame) {
    let text = help_page_text();
    let area = frame.area();
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help Page")
                .title_style(title_style()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_main_page(frame: &mut ratatui::Frame, app_state: &AppState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(area);

    draw_messages_list(frame, app_state, chunks[0]);

    draw_input_field(frame, app_state, chunks[1]);
}

fn draw_messages_list(frame: &mut ratatui::Frame, app_state: &AppState, rect: Rect) {
    let ui_state = app_state.lock_ui_state();
    let messages: Vec<ListItem> = app_state
        .lock_messages()
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|message| ListItem::new(format_message(message)))
        .collect();
    let border_style = match ui_state.focused {
        FocusedElement::MessageList => focused_border_style(),
        _ => unfocused_border_style(),
    };
    let messages_list = List::new(messages).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title_text(app_state))
            .title_style(title_style()),
    );
    frame.render_widget(messages_list, rect);
}

fn draw_input_field(frame: &mut ratatui::Frame, app_state: &AppState, rect: Rect) {
    let ui_state = app_state.lock_ui_state();
    let border_style = match ui_state.focused {
        FocusedElement::InputField => focused_border_style(),
        _ => unfocused_border_style(),
    };
    let input_field_state = ui_state.input_field_state();
    let text = input_field_state.text();
    let caret = match input_field_state.cursor() {
        Cursor::Caret(caret) => caret,
        _ => todo!(),
    };
    let text_with_cursor: Vec<Span> = if input_field_state.caret_is_at_end() {
        vec![Span::raw(text), Span::styled("_", caret_text_style())]
    } else {
        vec![
            Span::raw(&text[0..caret]),
            Span::styled(&text[caret..caret + 1], caret_text_style()),
            Span::raw(&text[caret + 1..]),
        ]
    };
    let input_field = Paragraph::new(Line::from(text_with_cursor))
        .block(
            Block::default()
                .borders(Borders::all())
                .border_style(border_style)
                .title_bottom(return_to_send()),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(input_field, rect);
}

fn title_text(app_state: &AppState) -> String {
    let second = Utc::now()
        .second()
        .wrapping_sub(app_state.start_date().second());
    match second / 3 % 2 {
        0 => "Welcome to Message Board, <Ctrl + H> for Help".into(),
        1 => format!("Server: {}", app_state.api().server_url()),
        _ => unreachable!(),
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

fn focused_border_style() -> Style {
    Style::new().fg(Color::Yellow)
}

fn unfocused_border_style() -> Style {
    Style::new().fg(Color::White)
}

fn help_page_text() -> &'static str {
    include_str!("help_page_text.txt")
}

fn caret_text_style() -> Style {
    Style::default().bg(Color::White).fg(Color::Black)
}
