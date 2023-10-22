use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Alignment, Buffer, Rect},
    style::{Style, Styled},
    text::Text,
    widgets::{Block, Paragraph, Widget},
};

use crate::gui::windows::WidgetWithHints;

#[derive(Clone)]
pub(crate) struct Button<'a> {
    pub(crate) label: String,
    pub(crate) label_style: Style,
    pub(crate) block: Block<'a>,
    /// Checks if an `Event` is the one that activates the button
    pub(crate) key_event: KeyEvent,
}

impl<'a> Button<'a> {
    pub(crate) fn set_label(&mut self, label: String) {
        self.label = label;
    }
    pub(crate) fn set_key_event(&mut self, key_event: KeyEvent) {
        self.key_event = key_event;
    }
    pub(crate) fn disable(&mut self) {
        self.set_key_event(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE));
    }
}

impl Widget for Button<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragarph = Paragraph::new(Text::styled(self.label, self.label_style))
            .alignment(Alignment::Center)
            .block(self.block);
        paragarph.render(area, buf);
    }
}

impl WidgetWithHints for Button<'_> {
    fn get_hints(&self) -> Vec<(KeyEvent, String)> {
        vec![(self.key_event, String::from("Press button"))]
    }
}

impl<'a> Styled for Button<'a> {
    type Item = Button<'a>;

    fn style(&self) -> Style {
        self.label_style
    }

    fn set_style(self, style: Style) -> Self::Item {
        Self {
            label: self.label,
            label_style: self.label_style.patch(style),
            block: self.block.set_style(style),
            key_event: self.key_event,
        }
    }
}
