use crossterm::event::Event;
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    widgets::{Block, Borders, List as TuiList, ListState, StatefulWidget, Widget},
};

use crate::{
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT,
};

#[derive(Clone, Default)]
pub(crate) struct List<'a> {
    pub(crate) state: ListState,
    pub(crate) items: Vec<String>,
    pub(crate) block: Block<'a>,
    pub(crate) style: Style,
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

        let item_num = self.items.len() - lower_bound;
        let mut items: Vec<String> = Vec::with_capacity(item_num);
        let mut heights: Vec<usize> = Vec::with_capacity(item_num);

        for item in &self.items[lower_bound..] {
            let text = textwrap::wrap(item, width);
            heights.push(text.len());
            items.push(text.join("\n"));
        }

        let inner = self.block.inner(area);
        let block_height = inner.height as usize;
        while items.len() > 0 {
            let total = heights.iter().sum::<usize>();
            if total >= block_height {
                if total - heights[0] > block_height {
                    items.remove(0);
                    heights.remove(0);
                } else {
                    let difference = total - block_height;
                    items[0] = items[0].split("\n").collect::<Vec<_>>()[difference..].join("\n");
                    break;
                }
            } else {
                break;
            }
        }

        let list = TuiList::new(items)
            .block(self.block)
            .style(self.style)
            .highlight_style(self.style.reversed());
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}

impl WidgetWithHints for List<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        Vec::new()
    }
}

impl FocusableWidget for List<'_> {
    fn make_focused(&mut self) {
        self.block = self.block.clone().title_style(self.style.reversed());
    }
}

impl SLSKWidget for List<'_> {}
