use std::rc::Rc;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Widget},
};
use tokio::sync::broadcast::Sender;
use tui_input::backend::crossterm::EventHandler;

use crate::{
    events::SLSKEvents,
    gui::widgets::{
        button::Button,
        input::{Input, InputType},
    },
    styles::STYLE_DEFAULT,
};

use super::{FocusableWidget, SLSKWidget, WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct LoginWindow<'a> {
    pub(crate) title: String,
    pub(crate) username_input: Input<'a>,
    pub(crate) password_input: Input<'a>,
    pub(crate) login_button: Button<'a, (String, String, Sender<SLSKEvents>), ()>,
    pub(crate) focus_index: u8,
}

impl Default for LoginWindow<'_> {
    fn default() -> Self {
        Self {
            title: String::from(" Login "),
            username_input: Input::default().title(String::from("Username")),
            password_input: Input::default()
                .title(String::from("Password"))
                .input_type(InputType::Password),
            login_button: Button {
                label: String::from("LOGIN"),
                label_style: STYLE_DEFAULT,
                block: Block::new().borders(Borders::ALL).style(STYLE_DEFAULT),
                event: Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                func: Rc::new(|s, (username, password, writer)| {
                    if !s.disabled && !username.is_empty() && !password.is_empty() {
                        let _ = writer.send(SLSKEvents::TryLogin { username, password });
                    }
                }),
                disabled: false,
            },
            focus_index: 0,
        }
    }
}

impl Widget for LoginWindow<'_> {
    fn render(mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let padding = (area.height - 9) / 2;
        let columns = Layout::new()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(area);
        let chunks = Layout::new()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(padding),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(padding),
            ])
            .split(columns[1]);

        render_widgets!(
            SELF: self,
            BUFFER: buf,
            0 = (self.username_input) => chunks[1],
            1 = (self.password_input) => chunks[2],
            2 = (self.login_button) => chunks[3],
        );
    }
}

impl WidgetWithHints for LoginWindow<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        self.get_widget(self.focus_index).get_hints()
    }
}

impl Window<'_> for LoginWindow<'_> {
    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn perform_action(&mut self, focus_index: u8, event: Event, write_queue: &Sender<SLSKEvents>) {
        match focus_index {
            0 => self.username_input.handle_event(&event),
            1 => self.password_input.handle_event(&event),
            2 => {
                (self.login_button.func)(
                    &self.login_button,
                    (
                        self.username_input.input_string.clone(),
                        self.password_input.input_string.clone(),
                        write_queue.clone(),
                    ),
                );
                None
            }
            _ => unimplemented!("perform_action({focus_index}, {event:?})"),
        };
    }

    fn number_of_widgets(&self) -> u8 {
        3
    }

    fn get_widget(&self, index: u8) -> &dyn SLSKWidget {
        match index {
            0 => &self.username_input,
            1 => &self.password_input,
            2 => &self.login_button,
            _ => unimplemented!(
                "There are only {} widgets, it's impossible to get the widget with index {index}",
                self.number_of_widgets()
            ),
        }
    }

    fn get_focused_index(&self) -> u8 {
        self.focus_index
    }

    fn set_focused_index(&mut self, index: u8) {
        self.focus_index = index;
    }
}
