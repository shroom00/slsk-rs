use std::fmt::Display;

use crossterm::event::Event;
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Style, Styled, Stylize, Modifier},
    text::{Line, Span, Masked},
    widgets::{Block, Borders, Paragraph, Widget},
};
use tui_input::{backend::crossterm::EventHandler, Input as TuiInput};

use crate::{
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT,
};


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

#[derive(Clone)]
pub(crate) struct Input<'a> {
    pub(crate) input: TuiInput,
    pub(crate) input_type: InputType,
    pub(crate) input_string: String,
    pub(crate) style: Style,
    pub(crate) block: Block<'a>,
    pub(crate) in_focus: bool,
}

impl Input<'_> {
    pub(crate) fn title(&mut self, title: String) -> Self {
        let mut new_input = self.clone();
        new_input.block = self.block.clone().title(title);
        new_input
    }

    pub(crate) fn input_type(&mut self, input_type: InputType) -> Self {
        let mut new_input = self.clone();
        new_input.input_type = input_type;
        new_input
    }

    pub(crate) fn clear(&mut self) {
        self.input_string = String::new();
        self.input.reset();
    }
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
                    .with_value(Masked::new(&self.input_string, '*').to_string())
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

impl Default for Input<'_> {
    fn default() -> Self {
        Self {
            input: Default::default(),
            input_type: Default::default(),
            input_string: Default::default(),
            style: STYLE_DEFAULT,
            block: Block::default().borders(Borders::ALL).on_black(),
            in_focus: false,
        }
    }
}

impl Widget for Input<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let input_width = area.width.max(3) - 3; // keep 2 for borders and 1 for cursor
        let input_scroll = self.input.visual_scroll(input_width as usize);

        let cursor = self.input.cursor();
        let raw_text = self.input.value().to_string();
        let line: Line;

        let indices: Vec<(usize, char)> = raw_text.char_indices().collect();

        let at_end = cursor == indices.len();
        if self.in_focus && at_end {
            line = Line::from(vec![
                Span::from(raw_text),
                Span::from(" ").set_style(self.style.add_modifier(Modifier::REVERSED)),
            ]);
        } else if self.in_focus {
            line = Line::from(vec![
                Span::from(&raw_text[..indices[cursor].0]),
                Span::from(indices[cursor].1.to_string()).set_style(self.style.add_modifier(Modifier::REVERSED)),
                Span::from(if cursor + 1 >= indices.len() {
                    ""
                } else {
                    &raw_text[indices[cursor + 1].0..]
                }),
            ]);
        } else {
            line = Line::from(raw_text);
        }

        let input_widget = Paragraph::new(line)
            .style(self.style)
            .scroll((0, input_scroll as u16))
            .block(self.block);
        input_widget.render(area, buf);
    }
}

impl WidgetWithHints for Input<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        Vec::new()
    }
}

impl FocusableWidget for Input<'_> {
    fn make_focused(&mut self) {
        self.block = self.block.clone().title_style(self.style.add_modifier(Modifier::REVERSED));
        self.in_focus = true;
    }
}

impl SLSKWidget for Input<'_> {}
