use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    style::{Styled, Stylize},
    widgets::{Block, Borders, Widget},
};

use crate::{
    gui::{
        styles::{STYLE_DEFAULT, STYLE_DEFAULT_HIGHLIGHT, STYLE_DEFAULT_LOW_CONTRAST},
        widgets::{
            button::Button,
            input::{Input, InputType},
        },
    },
    utils::invert_style,
};

use super::{WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct LoginWindow<'a> {
    pub(crate) title: String,
    pub(crate) username_input: Input<'a>,
    pub(crate) password_input: Input<'a>,
    pub(crate) login_button: Button<'a>,
    pub(crate) focus_index: u8,
}

impl Default for LoginWindow<'_> {
    fn default() -> Self {
        Self {
            title: String::from(" Login "),
            username_input: Input {
                input: Default::default(),
                input_type: Default::default(),
                input_string: Default::default(),
                style: STYLE_DEFAULT,
                block: Block::default()
                    .borders(Borders::ALL)
                    .title("Username")
                    .on_black(),
                in_focus: false,
            },
            password_input: Input {
                input: Default::default(),
                input_type: InputType::Password,
                input_string: Default::default(),
                style: STYLE_DEFAULT,
                block: Block::default()
                    .borders(Borders::ALL)
                    .title("Password")
                    .on_black(),
                in_focus: false,
            },
            login_button: Button {
                label: String::from("LOGIN"),
                label_style: STYLE_DEFAULT,
                block: Block::new().borders(Borders::ALL).style(STYLE_DEFAULT),
                key_event: KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            },
            focus_index: 0,
        }
    }
}

impl Widget for LoginWindow<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let padding = (area.height - 9) / 2;
        let columns = Layout::new()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(30),
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                ]
                .as_ref(),
            )
            .split(area);
        let chunks = Layout::new()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(padding),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(padding),
                ]
                .as_ref(),
            )
            .split(columns[1]);

        self.username_input.clone().render(chunks[1], buf);
        self.password_input.clone().render(chunks[2], buf);
        self.login_button.clone().render(chunks[3], buf);

        match self.focus_index {
            0 => {
                let mut focused_input = self.username_input;
                focused_input.block = focused_input
                    .block
                    .title_style(invert_style(focused_input.style));
                focused_input.style = STYLE_DEFAULT_LOW_CONTRAST;
                focused_input.in_focus = true;
                focused_input.render(chunks[1], buf);
            }
            1 => {
                let mut focused_input = self.password_input;
                focused_input.block = focused_input.block.title_style(STYLE_DEFAULT_HIGHLIGHT);
                focused_input.style = STYLE_DEFAULT_LOW_CONTRAST;
                focused_input.in_focus = true;
                focused_input.render(chunks[2], buf);
            }
            2 => {
                let mut focused_button = self.login_button;
                let button_style = invert_style(focused_button.style());
                focused_button = focused_button.set_style(button_style);
                focused_button.render(chunks[3], buf);
            }
            _ => (),
        }
    }
}

impl WidgetWithHints for LoginWindow<'_> {
    fn get_hints(&self) -> Vec<(KeyEvent, String)> {
        self.get_widget(self.focus_index).get_hints()
    }
}
