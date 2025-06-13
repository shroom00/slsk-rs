use std::rc::Rc;

use crossterm::event::Event;
use ratatui::{
    prelude::{Alignment, Buffer, Rect},
    style::{Style, Styled, Stylize},
    text::Text,
    widgets::{Block, Paragraph, Widget},
};

use crate::gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints};

#[derive(Clone)]
pub(crate) struct Button<'a, Args, Out>
where
    Args: Clone,
    Out: Clone,
{
    pub(crate) label: String,
    pub(crate) label_style: Style,
    pub(crate) block: Block<'a>,
    /// `event` is the one that activates the button
    pub(crate) event: Event,
    pub(crate) func: Rc<dyn for<'func> Fn(&Self, Args) -> Out>,
    pub(crate) disabled: bool,
}

impl<'a, Args: Clone, Out: Clone> Button<'a, Args, Out> {
    pub(crate) fn set_label(&mut self, label: String) {
        self.label = label;
    }

    pub(crate) fn disable(&mut self) {
        self.disabled = true;
    }

    pub(crate) fn enable(&mut self) {
        self.disabled = false;
    }
}

impl<Args: Clone, Out: Clone> Widget for Button<'_, Args, Out> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragarph = Paragraph::new(Text::styled(self.label, self.label_style))
            .alignment(Alignment::Center)
            .block(self.block);
        paragarph.render(area, buf);
    }
}

impl<Args: Clone, Out: Clone> WidgetWithHints for Button<'_, Args, Out> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        vec![(self.event.clone(), String::from("Press button"))]
    }
}

impl<Args: Clone, Out: Clone> FocusableWidget for Button<'_, Args, Out> {
    fn make_focused(&mut self) {
        let focus_style = self.label_style.reversed();
        self.label_style = self.label_style.patch(focus_style);
        self.block = self.block.clone().set_style(focus_style);
    }
}

impl<Args: Clone, Out: Clone> SLSKWidget for Button<'_, Args, Out> {}

impl<'a, Args: Clone, Out: Clone> Styled for Button<'a, Args, Out> {
    type Item = Button<'a, Args, Out>;

    fn style(&self) -> Style {
        self.label_style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        let style: Style = style.into();
        Self {
            label: self.label,
            label_style: self.label_style.patch(style),
            block: self.block.set_style(style),
            event: self.event,
            func: self.func,
            disabled: false,
        }
    }
}
