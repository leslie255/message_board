use std::sync::{Arc, Weak};

use chrono::{DateTime, Local};
use domtui::views::{InputField, MutView, ScreenBuilder, Size, Stack, ViewCell};
use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    prelude::Rect,
    style::{
        Color::{self, *},
        Modifier, Style,
    },
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

use crate::{state::AppState, utils::DynResult};

const INPUT_FIELD_TAG: &str = "input_field";
const MESSAGES_LIST_TAG: &str = "messages_list";

#[derive(Debug, Clone)]
pub struct UIState {
    app_state: Weak<AppState>,
    current_screen: Screen,
    main_screen: domtui::views::Screen<'static, Stack<(ViewCell<'static>, ViewCell<'static>)>>,
}

impl Default for UIState {
    fn default() -> Self {
        let mut main_screen = {
            let mut builder = ScreenBuilder::new();
            let root_view = Stack::vertical((
                builder.tagged_view_cell(MESSAGES_LIST_TAG, MessagesList::new(Weak::new())),
                builder.tagged_view_cell(INPUT_FIELD_TAG, MessageInputField::new(Weak::new())),
            ));
            builder.finish(root_view)
        };
        main_screen.focus_next();
        Self {
            app_state: Weak::default(),
            current_screen: Screen::default(),
            main_screen,
        }
    }
}

impl UIState {
    /// This function may only be called by `AppState`.
    pub fn messages_updated(&mut self) {
        log::info!("todo");
    }

    pub fn set_app_state(&mut self, app_state: Weak<AppState>) {
        self.app_state = app_state.clone();
        unsafe {
            self.main_screen
                .inspect_view_with_tag_unchecked::<(), MessageInputField>(INPUT_FIELD_TAG, |v| {
                    v.app_state = app_state.clone();
                })
                .unwrap();
            self.main_screen
                .inspect_view_with_tag_unchecked::<(), MessagesList>(MESSAGES_LIST_TAG, |v| {
                    v.app_state = app_state.clone();
                })
                .unwrap();
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum Screen {
    #[default]
    MainScreen,
    HelpScreen,
}

#[derive(Debug, Clone)]
pub struct MessageInputField {
    super_: InputField<'static>,
    app_state: Weak<AppState>,
}

impl MessageInputField {
    pub fn new(app_state: Weak<AppState>) -> Self {
        Self {
            super_: InputField::default()
                .placeholder("Send a message ...")
                .block_unfocused(borders(White))
                .block_focused(borders(LightYellow)),
            app_state,
        }
    }

    fn send_message(&mut self) {
        let app_state = self.app_state.upgrade().unwrap();
        let message = self.super_.content_mut().take_text();
        tokio::spawn(async move {
            let send_result = app_state.api().send_message(message.into()).await;
            if let Err(e) = send_result {
                log::error!("Error sending message: {e}")
            }
        });
    }
}

impl MutView for MessageInputField {
    fn render(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        self.super_.render(frame, area, is_focused);
    }

    fn on_focus(&mut self) {
        self.super_.on_focus()
    }

    fn on_unfocus(&mut self) {
        self.super_.on_unfocus()
    }

    fn is_focusable(&self) -> bool {
        self.super_.is_focusable()
    }

    fn on_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Press
            && key_event.modifiers == KeyModifiers::NONE
            && key_event.code == KeyCode::Enter
        {
            self.send_message();
        }
        self.super_.on_key_event(key_event);
    }

    fn preferred_size(&self) -> Option<Size> {
        Some(Size::new(u16::MAX, 3))
    }
}

#[derive(Debug, Clone)]
pub struct MessagesList {
    app_state: Weak<AppState>,
    scroll: i16,
}

impl MessagesList {
    pub fn new(app_state: Weak<AppState>) -> Self {
        Self {
            app_state,
            scroll: Default::default(),
        }
    }
}

impl MutView for MessagesList {
    fn render(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        // Area inside the borders.
        let area_inner = inner_area(area, 1);
        let mut lines = Vec::new();
        let app_state = self.app_state.upgrade().unwrap();
        let messages = app_state.lock_messages();
        let mut prev_date: DateTime<Local> = messages
            .front()
            .map(|m| m.date.into())
            .unwrap_or(DateTime::UNIX_EPOCH.into());
        for message in messages.iter() {
            let message_date: DateTime<Local> = message.date.into();
            if message_date.signed_duration_since(prev_date).num_seconds() >= 120 {
                lines.push(Line::styled(
                    message_date.format("[%Y-%m-%d %H:%M]").to_string(),
                    Style::new().fg(DarkGray),
                ));
            }
            prev_date = message_date;
            lines.push(Line::styled(
                message.content.as_ref(),
                Style::new().fg(White),
            ));
        }
        let extra_lines = lines.len().saturating_sub(usize::from(area_inner.height)) as i16;
        let scroll = u16::try_from(self.scroll.saturating_add(extra_lines)).unwrap_or(0);
        let block = Block::new()
            .borders(Borders::ALL)
            .style(Style::new().fg(if is_focused { LightYellow } else { White }))
            .title("Welcome to Message_Board")
            .title_style(Style::new().add_modifier(Modifier::BOLD));
        let pargraph = Paragraph::new(lines.to_vec())
            .scroll((scroll, 0))
            .block(block);
        frame.render_widget(pargraph, area);
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn on_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }

        // TODO: limit scrolling.
        use KeyCode::*;
        match (key_event.modifiers, key_event.code) {
            (KeyModifiers::NONE, Up) | (KeyModifiers::CONTROL, Char('p')) => {
                self.scroll -= 1;
            }
            (KeyModifiers::NONE, Down) | (KeyModifiers::CONTROL, Char('n')) => {
                self.scroll += 1;
            }
            (_, _) => (),
        }
    }
}

const fn inner_area(outer_area: Rect, border_width: u16) -> Rect {
    Rect {
        x: outer_area.x + border_width,
        y: outer_area.y + border_width,
        width: outer_area.width - border_width * 2,
        height: outer_area.height - border_width * 2,
    }
}

pub fn event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app_state: Arc<AppState>,
) -> DynResult<()> {
    let mut ui_state = app_state.lock_ui_state();

    'event_loop: loop {
        match &ui_state.current_screen {
            Screen::MainScreen => ui_state.main_screen.render(terminal)?,
            Screen::HelpScreen => {
                let paragraph = domtui::views::Paragraph::new(include_str!("help_page_text.txt"))
                    .block(borders(White).title("HELP (<ESC> TO GO BACK)"));
                domtui::render(terminal, paragraph)?
            }
        }
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue 'event_loop;
        }
        match event::read().unwrap() {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: _,
            }) => {
                break 'event_loop Ok(());
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: _,
            }) => {
                match &mut ui_state.current_screen {
                    screen @ Screen::MainScreen => *screen = Screen::HelpScreen,
                    screen @ Screen::HelpScreen => *screen = Screen::MainScreen,
                }
                continue 'event_loop;
            }
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: _,
            }) => {
                match &mut ui_state.current_screen {
                    Screen::MainScreen => (),
                    screen @ Screen::HelpScreen => *screen = Screen::MainScreen,
                }
                continue 'event_loop;
            }
            event => {
                match &mut ui_state.current_screen {
                    Screen::MainScreen => ui_state.main_screen.handle_event(event),
                    Screen::HelpScreen => (),
                }
                continue 'event_loop;
            }
        }
    }
}

fn borders(fg: Color) -> Block<'static> {
    Block::new()
        .borders(Borders::ALL)
        .style(Style::new().fg(fg))
}
