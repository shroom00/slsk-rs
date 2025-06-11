use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style, Stylize},
    text::Text,
    widgets::{block::Title, Block, Borders, Row, Table, Widget},
};
use std::ops::Range;
use std::{cmp::Ordering, sync::Arc};
use tokio::sync::RwLock;
use tui_input::{backend::crossterm::EventHandler, StateChanged};

use crate::{
    constants::{ByteSize, DownloadStatus, Percentage, Token},
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT, utils::num_as_str,
};

// TODO: Implement own EventHandler trait
pub(crate) const ITEM_INTERACTED: StateChanged = StateChanged { value: true, cursor: true };

#[derive(Clone, Debug)]
pub(crate) enum ColumnData {
    Empty,
    Usize(Arc<RwLock<usize>>),
    String(Arc<RwLock<String>>),
    ByteSize(Arc<RwLock<ByteSize>>),
    DownloadStatus(Arc<RwLock<DownloadStatus>>),
    Percentage(Arc<RwLock<Percentage>>),
    Token(u32),
}

impl PartialEq for ColumnData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Usize(l0), Self::Usize(r0)) => *l0.blocking_read() == *r0.blocking_read(),
            (Self::String(l0), Self::String(r0)) => *l0.blocking_read() == *r0.blocking_read(),
            (Self::ByteSize(l0), Self::ByteSize(r0)) => *l0.blocking_read() == *r0.blocking_read(),
            (Self::DownloadStatus(l0), Self::DownloadStatus(r0)) => {
                *l0.blocking_read() == *r0.blocking_read()
            }
            (Self::Percentage(l0), Self::Percentage(r0)) => {
                *l0.blocking_read() == *r0.blocking_read()
            }
            (Self::Token(l0), Self::Token(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Eq for ColumnData {}

impl PartialOrd for ColumnData {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            // Compare inner values directly when variants match
            (ColumnData::Empty, ColumnData::Empty) => Some(Ordering::Equal),
            (ColumnData::Usize(a), ColumnData::Usize(b)) => {
                Some(a.blocking_read().cmp(&b.blocking_read()))
            }
            (ColumnData::String(a), ColumnData::String(b)) => {
                Some(a.blocking_read().cmp(&b.blocking_read()))
            }
            (ColumnData::ByteSize(a), ColumnData::ByteSize(b)) => {
                Some(a.blocking_read().cmp(&b.blocking_read()))
            }
            (ColumnData::DownloadStatus(a), ColumnData::DownloadStatus(b)) => {
                Some(a.blocking_read().cmp(&b.blocking_read()))
            }
            (ColumnData::Percentage(a), ColumnData::Percentage(b)) => {
                Some(a.blocking_read().cmp(&b.blocking_read()))
            }
            (ColumnData::Token(a), ColumnData::Token(b)) => Some(a.cmp(&b)),
            // Fall back to string comparison for different variants
            _ => None,
        }
    }
}

impl Ord for ColumnData {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Compare inner values directly when variants match
            (ColumnData::Empty, ColumnData::Empty) => Ordering::Equal,
            (ColumnData::Usize(a), ColumnData::Usize(b)) => a.blocking_read().cmp(&b.blocking_read()),
            (ColumnData::String(a), ColumnData::String(b)) => a.blocking_read().cmp(&b.blocking_read()),
            (ColumnData::ByteSize(a), ColumnData::ByteSize(b)) => a.blocking_read().cmp(&b.blocking_read()),
            (ColumnData::DownloadStatus(a), ColumnData::DownloadStatus(b)) => {
                a.blocking_read().cmp(&b.blocking_read())
            }
            (ColumnData::Percentage(a), ColumnData::Percentage(b)) => a.blocking_read().cmp(&b.blocking_read()),
            (ColumnData::Token(a), ColumnData::Token(b)) => a.cmp(&b),
            // Fall back to string comparison for different variants
            _ => self.to_string().cmp(&other.to_string()),
        }
    }
}

impl ToString for ColumnData {
    fn to_string(&self) -> String {
        match self {
            ColumnData::Empty => String::new(),
            ColumnData::Usize(v) => num_as_str(*v.blocking_read()).to_string(),
            ColumnData::String(v) => v.blocking_read().to_string(),
            ColumnData::ByteSize(v) => v.blocking_read().to_string(),
            ColumnData::DownloadStatus(v) => v.blocking_read().to_string(),
            ColumnData::Percentage(v) => v.blocking_read().to_string(),
            ColumnData::Token(v) => v.to_string(),
        }
    }
}

impl From<usize> for ColumnData {
    fn from(value: usize) -> Self {
        ColumnData::Usize(Arc::new(RwLock::new(value)))
    }
}

impl From<String> for ColumnData {
    fn from(value: String) -> Self {
        ColumnData::String(Arc::new(RwLock::new(value)))
    }
}

impl From<ByteSize> for ColumnData {
    fn from(value: ByteSize) -> Self {
        ColumnData::ByteSize(Arc::new(RwLock::new(value)))
    }
}

impl TryInto<ByteSize> for ColumnData {
    type Error = ();

    fn try_into(self) -> Result<ByteSize, Self::Error> {
        if let Self::ByteSize(b) = self {
            Ok(ByteSize(b.blocking_read().0))
        } else {
            Err(())
        }
    }
}

impl From<DownloadStatus> for ColumnData {
    fn from(value: DownloadStatus) -> Self {
        ColumnData::DownloadStatus(Arc::new(RwLock::new(value)))
    }
}

impl From<Percentage> for ColumnData {
    fn from(value: Percentage) -> Self {
        ColumnData::Percentage(Arc::new(RwLock::new(value)))
    }
}

impl From<Token> for ColumnData {
    fn from(value: Token) -> Self {
        ColumnData::Token(value.0)
    }
}

impl ColumnData {
    pub(crate) fn from_vec<T>(vec: Vec<T>) -> Vec<Self>
    where
        ColumnData: From<T>,
    {
        vec.into_iter().map(Self::from).collect()
    }

    pub(crate) fn get_cell_data<T: 'static>(&self) -> Option<&Arc<RwLock<T>>> {
        match self {
            ColumnData::Usize(v)
                if std::any::TypeId::of::<T>() == std::any::TypeId::of::<usize>() =>
            {
                Some(unsafe { &*(v as *const Arc<RwLock<usize>> as *const Arc<RwLock<T>>) })
            }
            ColumnData::String(v)
                if std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() =>
            {
                Some(unsafe { &*(v as *const Arc<RwLock<String>> as *const Arc<RwLock<T>>) })
            }
            ColumnData::ByteSize(v)
                if std::any::TypeId::of::<T>() == std::any::TypeId::of::<ByteSize>() =>
            {
                Some(unsafe { &*(v as *const Arc<RwLock<ByteSize>> as *const Arc<RwLock<T>>) })
            }
            ColumnData::DownloadStatus(v)
                if std::any::TypeId::of::<T>() == std::any::TypeId::of::<DownloadStatus>() =>
            {
                Some(unsafe {
                    &*(v as *const Arc<RwLock<DownloadStatus>> as *const Arc<RwLock<T>>)
                })
            }
            ColumnData::Percentage(v)
                if std::any::TypeId::of::<T>() == std::any::TypeId::of::<Percentage>() =>
            {
                Some(unsafe { &*(v as *const Arc<RwLock<Percentage>> as *const Arc<RwLock<T>>) })
            }
            ColumnData::Token(_) => None,
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct TableItem {
    pub(crate) content: Vec<ColumnData>,
    pub(crate) children: Vec<TableItem>,
    is_open: bool,
}

impl Default for TableItem {
    fn default() -> Self {
        Self {
            content: Vec::new(),
            children: Vec::new(),
            is_open: false,
        }
    }
}

impl TableItem {
    pub(crate) fn new(content: Vec<ColumnData>, children: Vec<TableItem>) -> Self {
        Self {
            content,
            children,
            is_open: false,
        }
    }

    pub(crate) fn length(&self, filter_text: Option<&str>) -> usize {
        let mut length = 1;
        if !self.is_open {
            return length;
        }
        for child in &self.children {
            if !child.is_open {
                if let Some(filter_text) = filter_text {
                    if !child
                        .content
                        .iter()
                        .any(|content| content.to_string().to_lowercase().contains(filter_text))
                    {
                        length += 1;
                    }
                } else {
                    length += 1;
                }
            } else {
                length += child.length(filter_text);
            }
        }
        length
    }

    /// Compares content for sorting
    fn compare_content(&self, other: &Self, index: usize, is_ascending: bool) -> Ordering {
        let a_val = self.content.get(index);
        let b_val = other.content.get(index);

        if is_ascending {
            a_val.cmp(&b_val)
        } else {
            b_val.cmp(&a_val)
        }
    }

    /// Recursively sorts open children
    fn sort_open_children(&mut self, index: usize, is_ascending: bool) {
        if self.is_open {
            self.children
                .sort_by(|a, b| a.compare_content(b, index, is_ascending));

            for child in &mut self.children {
                child.sort_open_children(index, is_ascending);
            }
        }
    }

    pub(crate) fn open(mut self) -> Self {
        self.is_open = true;
        self
    }

    pub(crate) fn opened(mut self) -> Self {
        self.is_open = true;
        self
    }
}

#[derive(Clone)]
pub(crate) struct TableWidget<'a> {
    pub(crate) items: Vec<TableItem>,
    headers: Vec<String>,
    pub(crate) selected_row: usize,
    widths: Vec<Constraint>,
    style: Style,
    pub(crate) length: usize,
    sort: Option<(usize, Option<bool>)>,
    filter: Option<String>,
    filter_range: Option<Range<usize>>,
    block: Block<'a>,
}

impl TableWidget<'_> {
    pub(crate) fn new(
        mut headers: Vec<String>,
        items: Vec<TableItem>,
        filter_range: Option<Range<usize>>,
        widths: Option<Vec<Constraint>>,
    ) -> Self {
        let mut widths = widths.unwrap_or({
            let l = headers.len();
            vec![Constraint::Ratio(1, l as u32); l]
        });

        widths.insert(0, Constraint::Max(3));
        headers.insert(0, String::new());

        Self {
            length: items.iter().map(|item| item.length(None)).sum(),
            items,
            headers,
            selected_row: 0,
            widths,
            style: STYLE_DEFAULT,
            sort: None,
            filter: None,
            filter_range,
            block: Block::new().style(STYLE_DEFAULT).borders(Borders::ALL),
        }
    }

    pub(crate) fn insert_item(&mut self, item: TableItem) {
        self.length += item.length(self.filter.as_deref());
        self.items.push(item);
    }

    pub(crate) fn filter(&self) -> Option<&String> {
        self.filter.as_ref()
    }

    pub(crate) fn current_row(&self) -> Option<&TableItem> {
        if self.items.is_empty() {
            return None;
        }

        let mut current = 0;
        let mut stack = Vec::new();
        let mut current_iter = self.items.iter();

        loop {
            while let Some(item) = current_iter.next() {
                if current == self.selected_row {
                    return Some(item);
                }
                current += 1;
                // Push children onto stack if open
                if item.is_open && !item.children.is_empty() {
                    stack.push(current_iter);
                    current_iter = item.children.iter();
                }
            }

            // Continue with parent's siblings
            if let Some(parent_iter) = stack.pop() {
                current_iter = parent_iter;
            }
        }
    }

    pub(crate) fn next_row(&mut self) {
        if self.selected_row < self.length - 1 {
            self.selected_row += 1;
        }
    }

    pub(crate) fn previous_row(&mut self) {
        self.selected_row = self.selected_row.saturating_sub(1);
    }

    pub(crate) fn next_column(&mut self) {
        self.sort = match self.sort {
            Some((i, _)) => {
                if i < (self.headers.len() - 1) {
                    Some((i + 1, None))
                } else {
                    None
                }
            }
            None => Some((1, None)),
        }
    }

    pub(crate) fn previous_column(&mut self) {
        self.sort = match self.sort {
            Some((i, _)) => {
                if i != 1 {
                    Some((i - 1, None))
                } else {
                    None
                }
            }
            None => Some((self.headers.len() - 1, None)),
        }
    }

    /// Handles sort state changes and initiates sorting
    pub(crate) fn sort(&mut self) {
        if let Some((col_index, sort_order)) = self.sort.as_mut() {
            *sort_order = match sort_order {
                None => Some(true),
                Some(true) => Some(false),
                Some(false) => None,
            };

            if let Some(is_ascending) = sort_order {
                // Adjust for the [+] column if present (column 0 is the toggle)
                let sort_index = col_index.saturating_sub(1);

                // Sort all top-level items and their open children
                self.items
                    .sort_by(|a, b| a.compare_content(b, sort_index, *is_ascending));

                // Recursively sort open children
                for item in &mut self.items {
                    item.sort_open_children(sort_index, *is_ascending);
                }
            }
        }
    }

    fn set_length(&mut self) {
        self.length = 0;

        let mut stack = Vec::new();
        let mut current_iter = self.items.iter();

        loop {
            while let Some(item) = current_iter.next() {
                if let Some(text) = &self.filter {
                    if match &self.filter_range {
                        Some(filter_range) => item.content[filter_range.to_owned()].iter(),
                        None => item.content.iter(),
                    }
                    .any(|content: &ColumnData| content.to_string().to_lowercase().contains(text))
                    {
                        continue;
                    }
                }
                self.length += 1;

                // Push children onto stack if open
                if item.is_open && !item.children.is_empty() {
                    stack.push(current_iter);
                    current_iter = item.children.iter();
                }
            }

            // Continue with parent's siblings
            if let Some(parent_iter) = stack.pop() {
                current_iter = parent_iter;
            } else {
                break; // Done when stack is empty
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_filter(&mut self, text: String) {
        if !text.is_empty() {
            self.filter = Some(text.to_lowercase());
            self.set_length();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn filter_range(mut self, filter_range: Option<Range<usize>>) -> Self {
        self.filter_range = filter_range;
        self
    }

    #[allow(dead_code)]
    pub(crate) fn is_filtered(&self) -> bool {
        self.filter.is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn remove_filter(&mut self) {
        self.filter = None;
        self.set_length();
    }

    fn make_headers(&self) -> Vec<Text> {
        match self.sort {
            Some((i, order)) => {
                let mut headers: Vec<Text> = self
                    .headers
                    .iter()
                    .map(|h| Text::from(h.as_str()).style(self.style.add_modifier(Modifier::BOLD)))
                    .collect();
                match order {
                    Some(order) => {
                        headers[i] = Text::from(format!(
                            "{} {}",
                            if order { '↓' } else { '↑' },
                            self.headers[i]
                        ))
                        .style(self.style.reversed().add_modifier(Modifier::BOLD))
                    }
                    None => {
                        headers[i] = headers[i]
                            .clone()
                            .style(self.style.reversed().add_modifier(Modifier::BOLD))
                    }
                }

                headers
            }
            _ => self
                .headers
                .iter()
                .map(|h| Text::from(h.as_str()).style(self.style.add_modifier(Modifier::BOLD)))
                .collect(),
        }
    }

    /// Opens or closes the currently selected item
    pub(crate) fn toggle_item(&mut self) {
        let mut current_row = 0;
        let mut stack = vec![self.items.iter_mut()];
        let target_row = self.selected_row;

        while let Some(items_iter) = stack.last_mut() {
            if let Some(item) = items_iter.next() {
                if current_row == target_row {
                    if !item.children.is_empty() {
                        if item.is_open {
                            self.length -= item.length(self.filter.as_deref()) - 1;
                            item.is_open = !item.is_open;
                        } else {
                            item.is_open = !item.is_open;
                            self.length += item.length(self.filter.as_deref()) - 1;
                        }
                    } else {
                        item.is_open = !item.is_open;
                    }
                    return;
                }

                current_row += 1;

                if item.is_open {
                    stack.push(item.children.iter_mut());
                }
            } else {
                stack.pop();
            }
        }
    }

    fn rows(&self, offset: u16, height: u16) -> Vec<Row> {
        let mut current_row = 0u16;
        let mut rows = Vec::new();
        let mut stack = Vec::new();
        let mut current_iter = self.items.iter();
        let mut rows_rendered = 0;

        loop {
            while let Some(item) = current_iter.next() {
                if rows_rendered >= height {
                    break;
                } else if current_row >= offset {
                    if let Some(text) = &self.filter {
                        if match &self.filter_range {
                            Some(filter_range) => item.content[filter_range.clone()].iter(),
                            None => item.content.iter(),
                        }
                        .any(|content| content.to_string().to_lowercase().contains(text))
                        {
                            continue;
                        }
                    }
                    let mut content: Vec<String> =
                        item.content.iter().map(|d| d.to_string()).collect();
                    content[0] = "  ".repeat(stack.len()) + &content[0];

                    let toggle = if item.children.is_empty() {
                        "  "
                    } else if item.is_open {
                        "[-]"
                    } else {
                        "[+]"
                    };
                    content.insert(0, toggle.to_string());

                    rows.push(Row::new(content).style(
                        if self.selected_row == current_row as usize {
                            self.style.add_modifier(Modifier::REVERSED)
                        } else {
                            self.style
                        },
                    ));
                    rows_rendered += 1;
                }
                current_row += 1;

                if item.is_open && !item.children.is_empty() {
                    stack.push(current_iter);
                    current_iter = item.children.iter();
                }
            }

            if let Some(parent_iter) = stack.pop() {
                current_iter = parent_iter;
            } else {
                break;
            }
        }

        rows
    }

    fn get_cell_data<'a, T: 'static>(&self, cell: &'a ColumnData) -> Option<&'a Arc<RwLock<T>>> {
        cell.get_cell_data()
    }

    // Type-safe access to cell data
    #[allow(dead_code)]
    pub(crate) fn get_current_cell_data<T: 'static>(&self, col: usize) -> Option<&Arc<RwLock<T>>> {
        let item = self.current_row()?;
        let cell = item.content.get(col)?;
        self.get_cell_data(cell)
    }
}

impl<'a> TableWidget<'a> {
    pub(crate) fn title<T>(&self, title: T) -> Self
    where
        T: Into<Title<'a>>,
    {
        let mut table = self.clone();
        table.block = table.block.title(title);
        table
    }
}

impl Widget for TableWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let height = area.height - 3;
        let offset = (self.selected_row as u16 / height) * height;
        let rows = self.rows(offset, height);

        Table::new(rows, self.widths.clone())
            .header(Row::new(self.make_headers()))
            .widths(&self.widths)
            .block(self.block.clone())
            .render(area, buf);
    }
}

impl FocusableWidget for TableWidget<'_> {
    fn make_focused(&mut self) {
        self.block = self.block.clone().border_style(self.style.reversed());
    }
}

impl WidgetWithHints for TableWidget<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        // TODO: add hint for handling filtering
        vec![
            (
                Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
                String::from("Select previous row"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
                String::from("Select next row"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
                String::from("Select previous column"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
                String::from("Select next column"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
                String::from("Sort column"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                String::from("Open/Close table"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
                String::from("Interact"),
            ),
        ]
    }
}

impl SLSKWidget for TableWidget<'_> {}

impl EventHandler for TableWidget<'_> {
    fn handle_event(&mut self, evt: &Event) -> Option<tui_input::StateChanged> {
        if let Event::Key(key) = evt {
            if key.modifiers == KeyModifiers::NONE {
                if key.code == KeyCode::Up {
                    self.previous_row();
                } else if key.code == KeyCode::Down {
                    self.next_row();
                } else if key.code == KeyCode::Enter {
                    self.toggle_item();
                } else if key.code == KeyCode::Left {
                    self.previous_column();
                } else if key.code == KeyCode::Right {
                    self.next_column();
                } else if let KeyCode::Char(ch) = key.code {
                    if (ch == 's') | (ch == 'S') {
                        self.sort();
                    }
                }
                // TODO: add handling for filtering
            } else if key.modifiers == KeyModifiers::SHIFT {
                if key.code == KeyCode::Enter {
                    return Some(ITEM_INTERACTED);
                }
            }
        }
        None
    }
}
