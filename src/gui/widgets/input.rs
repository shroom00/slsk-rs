use std::fmt::Display;

use crossterm::event::Event;
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Style, Styled},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};
use tui_input::{backend::crossterm::EventHandler, Input as TuiInput};

use crate::{utils::{invert_style, mask_string}, gui::windows::WidgetWithHints};

#[derive(Clone)]
pub(crate) enum InputType {
    Standard,
    Password,
}

impl Default for InputType {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Default, Clone)]
pub(crate) struct Input<'a> {
    pub(crate) input: TuiInput,
    pub(crate) input_type: InputType,
    pub(crate) input_string: String,
    pub(crate) style: Style,
    pub(crate) block: Block<'a>,
    pub(crate) in_focus: bool,
}

impl EventHandler for Input<'_> {
    fn handle_event(&mut self, evt: &Event) -> Option<tui_input::StateChanged> {
        let out = self.input.handle_event(evt);
        let mut temp_input = TuiInput::new(self.input_string.clone());
        temp_input.handle_event(evt);
        self.input_string = temp_input.value().to_string();
        match self.input_type {
            InputType::Standard => (),
            InputType::Password => {
                self.input = self
                    .input
                    .clone()
                    .with_value(mask_string(&self.input_string))
                    .with_cursor(self.input.visual_cursor());
            }
        };
        out
    }
}

impl Display for Input<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.input.fmt(f)
    }
}

impl Widget for Input<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let input_width = area.width.max(3) - 3; // keep 2 for borders and 1 for cursor
        let input_scroll = self.input.visual_scroll(input_width as usize);
        let cursor = self.input.visual_cursor();
        let raw_text = self.input.value().to_string();
        let (before, mut after) = raw_text.split_at(cursor);
        if after == "" {
            after = " "
        }
        let (at, after) = after.split_at(1);
        let line = Line::from(if self.in_focus {
            vec![
                Span::from(before),
                Span::from(at).set_style(invert_style(self.style)),
                Span::from(after),
            ]
        } else {
            vec![Span::from(before), Span::from(at), Span::from(after)]
        });
        let input_widget = Paragraph::new(line)
            .style(self.style)
            .scroll((0, input_scroll as u16))
            .block(self.block);
        input_widget.render(area, buf);
    }
}

impl WidgetWithHints for Input<'_> {
    fn get_hints(&self) -> Vec<(crossterm::event::KeyEvent, String)> {
        vec![]
    }
}