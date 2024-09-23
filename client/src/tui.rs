#![allow(dead_code)]

use std::{
    fmt::{self, Debug, Display},
    io::{stdout, Stdout},
    sync::Arc,
};

use chrono::{Timelike, Utc};
use copypasta::ClipboardContext;
use interface::Message;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        self,
        event::{
            self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
        },
    },
    layout::{Constraint, Direction, Layout, Position, Rect},
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

#[derive(Clone, Copy)]
pub struct DotDot;

impl Debug for DotDot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "..")
    }
}

impl Display for DotDot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "..")
    }
}

/// State of UI elements.
pub struct UIState {
    focused: FocusedElement,
    input_field_state: InputFieldState,
    messages_list_area: Option<Rect>,
    input_field_area: Option<Rect>,
    is_in_help_page: bool,
    /// `None` if clipboard_context can't be initialized.
    clipboard_context: Option<ClipboardContext>,
}

impl Debug for UIState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UIState")
            .field("focused", &self.focused)
            .field("input_field_state", &self.input_field_state)
            .field("messages_list_area", &self.messages_list_area)
            .field("input_field_area", &self.input_field_area)
            .field(
                "is_in_help_page",
                &self.clipboard_context.as_ref().map(|_| DotDot),
            )
            .finish()
    }
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
    pub fn new() -> Self {
        Self {
            focused: FocusedElement::default(),
            input_field_state: InputFieldState::default(),
            messages_list_area: None,
            input_field_area: None,
            is_in_help_page: false,
            clipboard_context: ClipboardContext::new()
                .inspect_err(|e| log::error!("Error initializing clipboard context: {e}"))
                .ok(),
        }
    }

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

    pub fn copy(&mut self) {
        match self.clipboard_context.as_mut() {
            Some(clipboard) => {
                self.input_field_state
                    .copy(clipboard)
                    .unwrap_or_else(|e| log::error!("Error copying: {e}"));
            }
            None => {
                log::error!(
                    "Cannot copy because `ClipboardContext` could not be initialized earlier"
                );
            }
        }
    }

    pub fn paste(&mut self) {
        match self.clipboard_context.as_mut() {
            Some(clipboard) => {
                self.input_field_state.paste(clipboard);
            }
            None => {
                log::error!(
                    "Cannot paste because `ClipboardContext` could not be initialized earlier"
                );
            }
        }
    }
}

pub fn setup_terminal() -> Terminal<CrosstermBackend<Stdout>> {
    use crossterm::event::EnableMouseCapture;
    crossterm::execute!(stdout(), EnableMouseCapture).unwrap();
    ratatui::init()
}

pub fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) {
    use crossterm::event::DisableMouseCapture;
    terminal.show_cursor().unwrap();
    crossterm::execute!(stdout(), DisableMouseCapture).unwrap();
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
        match event::read()? {
            Event::Key(key_event) => {
                match global_handle_key(&app_state, key_event) {
                    GlobalHandleKeyResult::Continue => continue 'event_loop,
                    GlobalHandleKeyResult::Break => break 'event_loop Ok(()),
                    GlobalHandleKeyResult::Pass => (),
                }
                let focused_element = app_state.lock_ui_state().focused;
                match focused_element {
                    FocusedElement::InputField => input_field_handle_key(&app_state, key_event),
                    FocusedElement::MessageList => message_list_handle_key(&app_state, key_event),
                }
            }
            Event::Mouse(mouse_event) => global_handle_mouse(&app_state, mouse_event),
            _ => continue 'event_loop,
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

pub fn global_handle_key(app_state: &AppState, key_event: KeyEvent) -> GlobalHandleKeyResult {
    match (key_event.modifiers, key_event.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('h')) => {
            app_state.lock_ui_state().is_in_help_page = true;
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app_state.lock_ui_state().is_in_help_page = false;
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::NONE, KeyCode::Tab) => {
            app_state.lock_ui_state().focus_next();
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            app_state.lock_ui_state().copy();
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
            app_state.lock_ui_state().paste();
            GlobalHandleKeyResult::Continue
        }
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => GlobalHandleKeyResult::Break,
        _ => GlobalHandleKeyResult::Pass,
    }
}

pub fn global_handle_mouse(app_state: &AppState, mouse_event: MouseEvent) {
    #[allow(clippy::single_match)]
    match mouse_event.kind {
        MouseEventKind::Up(MouseButton::Left) => {
            let mut ui_state = app_state.lock_ui_state();
            let position = Position::new(mouse_event.column, mouse_event.row);
            if ui_state
                .messages_list_area
                .is_some_and(|area| area.contains(position))
            {
                ui_state.focused = FocusedElement::MessageList;
            } else if ui_state
                .input_field_area
                .is_some_and(|area| area.contains(position))
            {
                ui_state.focused = FocusedElement::InputField;
            }
        }
        _ => (),
    }
}

/// `unwrap_or` but `const`.
const fn unwrap_or<T: Copy>(default: T, value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => default,
    }
}

pub const CONTROL_SHIFT: KeyModifiers = unwrap_or(
    KeyModifiers::NONE,
    KeyModifiers::from_bits(KeyModifiers::CONTROL.bits() | KeyModifiers::SHIFT.bits()),
);

pub fn input_field_handle_key(app_state: &Arc<AppState>, key: KeyEvent) {
    macro input_field_state_mut() {
        app_state.lock_ui_state().input_field_state_mut()
    }
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Left) | (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
            input_field_state_mut!().caret_left()
        }
        (KeyModifiers::NONE, KeyCode::Right) | (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
            input_field_state_mut!().caret_right()
        }
        (KeyModifiers::CONTROL, KeyCode::Left | KeyCode::Char('a')) => {
            input_field_state_mut!().caret_left_end();
        }
        (KeyModifiers::CONTROL, KeyCode::Right | KeyCode::Char('e')) => {
            input_field_state_mut!().caret_right_end();
        }
        (KeyModifiers::SHIFT, KeyCode::Left) => {
            input_field_state_mut!().select_left();
        }
        (KeyModifiers::SHIFT, KeyCode::Right) => {
            input_field_state_mut!().select_right();
        }
        (CONTROL_SHIFT, KeyCode::Left) => {
            input_field_state_mut!().select_left_end();
        }
        (CONTROL_SHIFT, KeyCode::Right) => {
            input_field_state_mut!().select_right_end();
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => input_field_state_mut!().delete_backward(),
        (KeyModifiers::NONE, KeyCode::Delete) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            input_field_state_mut!().delete_forward();
        }
        (KeyModifiers::NONE, KeyCode::Char(char)) => input_field_state_mut!().insert(char),
        (KeyModifiers::SHIFT, KeyCode::Char(char)) => {
            // FIXME: Respect more advanced keyboard layout (such as those with AltGr).
            let mut ui_state = app_state.lock_ui_state();
            for char in char.to_uppercase() {
                ui_state.input_field_state_mut().insert(char);
            }
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

fn draw_messages_list(frame: &mut ratatui::Frame, app_state: &AppState, area: Rect) {
    let mut ui_state = app_state.lock_ui_state();
    ui_state.messages_list_area = Some(area);
    let messages: Vec<ListItem> = app_state
        .lock_messages()
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
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
    frame.render_widget(messages_list, area);
}

fn draw_input_field(frame: &mut ratatui::Frame, app_state: &AppState, area: Rect) {
    let mut ui_state = app_state.lock_ui_state();
    ui_state.input_field_area = Some(area);
    let is_focused = matches!(ui_state.focused, FocusedElement::InputField);
    let border_style = if is_focused {
        focused_border_style()
    } else {
        unfocused_border_style()
    };
    let input_field_state = ui_state.input_field_state();
    let text = input_field_state.text();
    let bottom_text = if is_focused && !text.is_empty() {
        return_to_send()
    } else {
        ""
    };
    let input_field = input_field_paragraph(is_focused, input_field_state)
        .block(
            Block::default()
                .borders(Borders::all())
                .border_style(border_style)
                .title_bottom(bottom_text),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(input_field, area);
}

fn title_text(app_state: &AppState) -> String {
    let second = Utc::now()
        .second()
        .wrapping_sub(app_state.start_date().second());
    match second / 3 % 2 {
        0 => "<Ctrl + H> for Help".into(),
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

fn caret_color() -> Color {
    Color::White
}

fn caret_bg() -> Style {
    Style::default().bg(caret_color())
}

fn selection_bg() -> Style {
    Style::default().bg(Color::LightBlue)
}

fn input_field_placeholder(is_focused: bool) -> Paragraph<'static> {
    if is_focused {
        Paragraph::new(Line::from(vec![
            Span::styled("S", caret_bg().fg(Color::DarkGray)),
            Span::styled("end a message...", Style::new().fg(Color::DarkGray)),
        ]))
    } else {
        Paragraph::new("Send a message...").style(Style::new().fg(Color::DarkGray))
    }
}

fn input_field_paragraph(is_focused: bool, input_field_state: &InputFieldState) -> Paragraph {
    let text = input_field_state.text();
    if text.is_empty() {
        return input_field_placeholder(is_focused);
    }
    if !is_focused {
        return Paragraph::new(text);
    }
    match input_field_state.cursor() {
        Cursor::Caret(caret) => {
            if input_field_state.caret_is_at_end() {
                return Paragraph::new(Line::from(vec![
                    Span::raw(text),
                    Span::styled(".", caret_bg().fg(caret_color())),
                ]));
            }
            Paragraph::new(Line::from(vec![
                Span::raw(&text[0..caret]),
                Span::styled(&text[caret..caret + 1], caret_bg().fg(Color::Black)),
                Span::raw(&text[caret + 1..]),
            ]))
        }
        Cursor::Selection(range) => Paragraph::new(Line::from(vec![
            Span::raw(&text[0..range.start]),
            Span::styled(&text[range], selection_bg().fg(Color::Black)),
            Span::raw(&text[range.end..]),
        ])),
    }
}
