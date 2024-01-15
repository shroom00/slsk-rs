use crossterm::event::Event;
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, List as TuiList, ListItem, ListState, StatefulWidget, Widget},
};

use crate::{
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT,
};

#[derive(Clone)]
pub(crate) struct List<'a> {
    state: ListState,
    items: Vec<String>,
    block: Block<'a>,
    style: Style,
}

impl List<'_> {
    pub(crate) fn new(items: Vec<String>) -> Self {
        Self {
            state: Default::default(),
            items,
            block: Block::default().borders(Borders::ALL).style(STYLE_DEFAULT),
            style: STYLE_DEFAULT,
        }
    }
}

impl Widget for List<'_> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let lower_bound = self.items.len().saturating_sub(area.height as usize);
        let width = area.width.saturating_sub(3) as usize; // - 3 to account for possible borders (-2 caused one character to overflow?)

        let items: Vec<ListItem> = self.items[lower_bound..]
            .iter()
            .map(|item| ListItem::new(textwrap::fill(item, width)))
            .collect();

        let list = TuiList::new(items)
            .block(self.block)
            .style(self.style)
            .highlight_style(self.style.reversed());
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}

impl WidgetWithHints for List<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        vec![]
    }
}

impl FocusableWidget for List<'_> {
    fn make_focused(&mut self) {
        self.block = self.block.clone().title_style(self.style.reversed());
    }
}

impl SLSKWidget for List<'_> {}
