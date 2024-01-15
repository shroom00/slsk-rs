use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Constraint,
    prelude::{Buffer, Rect},
    style::{Style, Modifier, Stylize},
    widgets::{Block, Borders, Row, StatefulWidget, Table as TuiTable, TableState, Widget},
};
use tui_input::backend::crossterm::EventHandler;

use crate::{
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT,
};

use super::SelectItem;

#[derive(Clone)]
pub(crate) struct Table<'a> {
    pub(crate) header: Option<Row<'a>>,
    pub(crate) rows: Vec<Row<'a>>,
    pub(crate) widths: &'a [Constraint],
    pub(crate) state: TableState,
    pub(crate) style: Style,
    pub(crate) block: Block<'a>,
    pub(crate) focused: bool,
}

impl<'a> Table<'a> {
    pub(crate) fn new(header: Option<Row<'a>>, rows: Vec<Row<'a>>) -> Self {
        let mut table = Table::default();
        table.header = match header {
            Some(header) => Some(header.bottom_margin(1)),
            None => header,
        };
        table.rows = rows;
        table
    }

    pub(crate) fn widths(&self, widths: &'a [Constraint]) -> Self {
        let mut new = self.clone();
        new.widths = widths;
        new
    }
}

impl SelectItem for Table<'_> {
    fn get_index(&self) -> Option<usize> {
        // self.highlighted_index
        self.state.selected()
    }

    fn set_index(&mut self, index: Option<usize>) {
        // self.highlighted_index = index;
        self.state.select(index);
    }

    fn select_previous(&mut self) {
        let mut index = self.get_index();
        index = match index {
            Some(n) => {
                if n == 0 {
                    None
                } else {
                    Some(n - 1)
                }
            }
            None => None,
        };
        self.set_index(index);
    }

    fn select_next(&mut self) {
        let mut index = self.get_index();
        index = match index {
            Some(n) => Some(n.saturating_add(1).min(self.rows.len().saturating_sub(1))),
            None => Some(0),
        };
        self.set_index(index);
    }
}

impl<'a> Widget for Table<'a> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        // This assumes top and bottom borders are present
        // 4 is top border, header, header spacing, bottom border
        // 2 is top border, bottom border
        let height = (area.height - {
            if self.header.is_some() {
                4
            } else {
                2
            }
        }) as usize;
        *self.state.offset_mut() = (self.state.selected().unwrap_or_default() / height) * height;

        let mut table = TuiTable::new(self.rows)
            .block(self.block)
            .widths(self.widths)
            .highlight_style(self.style.reversed());
        if self.header.is_some() {
            table = table.header(self.header.unwrap()).column_spacing(1);
        }

        StatefulWidget::render(table, area, buf, &mut self.state);
    }
}

impl Default for Table<'_> {
    fn default() -> Self {
        Self {
            header: Some(Row::new(["Placeholder Table"]).bottom_margin(1)),
            rows: vec![],
            widths: &[],
            state: Default::default(),
            style: STYLE_DEFAULT,
            block: Block::new().style(STYLE_DEFAULT).borders(Borders::ALL),
            focused: false,
        }
    }
}

impl WidgetWithHints for Table<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        vec![
            (
                Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
                String::from("Select previous row"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
                String::from("Select next row"),
            ),
        ]
    }
}

impl FocusableWidget for Table<'_> {
    fn make_focused(&mut self) {
        self.block = self.block.clone().border_style(self.style.reversed());
        self.focused = true;
    }
}

impl SLSKWidget for Table<'_> {}

impl EventHandler for Table<'_> {
    fn handle_event(&mut self, evt: &crossterm::event::Event) -> Option<tui_input::StateChanged> {
        if let Event::Key(key) = evt {
            if key.modifiers == KeyModifiers::NONE {
                if key.code == KeyCode::Up {
                    self.select_previous();
                } else if key.code == KeyCode::Down {
                    self.select_next();
                }
            }
        }
        None
    }
}
