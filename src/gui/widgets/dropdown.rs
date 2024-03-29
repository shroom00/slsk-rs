use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Alignment, Buffer, Rect},
    style::{Modifier, Style},
    text::Text,
    widgets::{
        block::Position, Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget,
        Widget, Clear,
    },
};
use tui_input::backend::crossterm::EventHandler;

use crate::{
    gui::{
        windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
        WINDOW_RESOLUTION,
    },
    styles::STYLE_DEFAULT,
};

use super::{input::Input, SelectItem};

#[derive(Clone)]
pub(crate) enum DropdownHeader<'a> {
    Search(Input<'a>),
    Title(&'a str),
}

pub(crate) trait DropwdownTrait: SelectItem {
    fn get_children(&self) -> &Vec<DropdownItem>;
    fn mut_get_children(&mut self) -> &mut Vec<DropdownItem>;
    fn selected(&self) -> Option<usize>;
    fn is_open(&self) -> bool;
    fn open(&mut self);
    fn close(&mut self);
    fn to_list(&self) -> Option<List> {
        if self.get_children().is_empty() {
            None
        } else {
            Some(
                List::new(
                    self.get_children()
                        .iter()
                        .map(|item| {
                            let text = item.text.clone();
                            ListItem::new(text)
                        })
                        .collect::<Vec<ListItem>>(),
                )
                .block(Block::default().borders(Borders::ALL)),
            )
        }
    }

    /// Returns (width, height)
    fn dimensions(&self) -> (usize, usize) {
        let mut width = 0;
        let mut height = 2;

        for item in self.get_children() {
            let text: Text<'_> = item.text.clone().into();
            width = width.max(text.width());
            height += text.height();
        }
        (width + 2, height)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DropdownItem {
    text: String,
    children: Vec<DropdownItem>,
    is_open: bool,
    selected: usize,
}

impl DropdownItem {
    pub(crate) fn new<T>(text: T, children: Vec<DropdownItem>) -> Self
    where
        T: Into<String>,
    {
        Self {
            text: text.into(),
            children,
            ..Default::default()
        }
    }

    pub(crate) fn empty<T>(text: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            text: text.into(),
            children: vec![],
            ..Default::default()
        }
    }

    pub(crate) fn flat<T>(text: T, children: Vec<String>) -> Self
    where
        T: Into<String>,
    {
        Self {
            text: text.into(),
            children: children
                .iter()
                .map(|child| DropdownItem::empty(child))
                .collect(),
            is_open: false,
            selected: 0,
        }
    }
}

impl Default for DropdownItem {
    fn default() -> Self {
        Self {
            text: String::from("DropdownItem"),
            children: vec![],
            is_open: false,
            selected: 0,
        }
    }
}

impl DropwdownTrait for DropdownItem {
    fn get_children(&self) -> &Vec<DropdownItem> {
        &self.children
    }

    fn mut_get_children(&mut self) -> &mut Vec<DropdownItem> {
        &mut self.children
    }

    fn selected(&self) -> Option<usize> {
        match self.is_open() {
            true => Some(self.selected),
            false => None,
        }
    }

    fn open(&mut self) {
        self.is_open = true;
    }

    fn close(&mut self) {
        self.is_open = false
    }

    fn is_open(&self) -> bool {
        self.is_open
    }
}

impl SelectItem for DropdownItem {
    fn get_index(&self) -> Option<usize> {
        self.selected()
    }

    fn set_index(&mut self, index: Option<usize>) {
        if index.is_some() {
            self.selected = index.unwrap();
        }
    }

    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1)
    }

    fn select_next(&mut self) {
        self.selected = self
            .selected
            .saturating_add(1)
            .min(self.children.len().saturating_sub(1));
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum DropdownAction {
    Open,
    Close,
    Next,
    Previous,
    Click,
}

#[derive(Clone)]
pub(crate) struct Dropdown<'a> {
    pub(crate) header: DropdownHeader<'a>,
    pub(crate) children: Vec<DropdownItem>,
    pub(crate) selected: usize,
    pub(crate) style: Style,
    pub(crate) is_open: bool,
    pub(crate) in_focus: bool,
    pub(crate) depth: usize,
    pub(crate) fetched_text: Option<String>,
}

impl<'a> Dropdown<'a> {
    pub(crate) fn new(header: DropdownHeader<'a>, children: Vec<DropdownItem>) -> Self {
        Self {
            header,
            children,
            ..Default::default()
        }
    }
}

impl Dropdown<'_> {
    fn alter_dropdown<'a>(&mut self, action: DropdownAction) {
        let act_on_root = if (action == DropdownAction::Open) | (action == DropdownAction::Click) {
            self.depth == 0
        } else {
            self.depth <= 1
        };

        if act_on_root {
            match action {
                DropdownAction::Open => {
                    self.is_open = true;
                    self.depth = 1;
                }
                DropdownAction::Close => {
                    self.is_open = false;
                    self.depth = 0;
                }
                DropdownAction::Next => {
                    if self.is_open {
                        self.selected = self
                            .selected
                            .saturating_add(1)
                            .min(self.children.len().saturating_sub(1));
                    }
                }
                DropdownAction::Previous => {
                    if self.is_open {
                        self.selected = self.selected.saturating_sub(1)
                    }
                }
                DropdownAction::Click => {
                    self.fetched_text = match &self.header {
                        DropdownHeader::Search(input) => Some(input.input_string.clone()),
                        DropdownHeader::Title(_) => None,
                    }
                }
            }
            return;
        };

        if self.children.is_empty() {
            return;
        }

        let mut child = &mut self.children[self.selected];
        let mut current_depth = 1;
        loop {
            let result: bool;
            let empty = child.children.is_empty();
            if current_depth < self.depth {
                if (current_depth == self.depth - 1)
                    && (action != DropdownAction::Open)
                    && (action != DropdownAction::Click)
                {
                    match action {
                        DropdownAction::Open | DropdownAction::Click => unimplemented!(),
                        DropdownAction::Close => {
                            child.close();
                            self.depth -= 1;
                        }
                        DropdownAction::Next => child.select_next(),
                        DropdownAction::Previous => child.select_previous(),
                    }
                    result = true;
                } else if empty {
                    result = true;
                } else {
                    current_depth += 1;
                    result = false;
                }
            } else {
                if action == DropdownAction::Open {
                    if !empty && !child.is_open {
                        child.open();
                        self.depth += 1;
                    }
                } else {
                    // if action == Dropdown::Click
                    self.fetched_text = Some(child.text.clone());
                }
                result = true;
            };

            match result {
                true => break,
                false => child = &mut child.children[child.selected],
            }
        }
    }

    // Returns the text of the currently selected item (assuming it's not the root)
    pub(crate) fn click(&mut self) {
        self.alter_dropdown(DropdownAction::Click);
    }
}

impl Default for Dropdown<'_> {
    fn default() -> Self {
        Self {
            header: DropdownHeader::Title("Dropdown"),
            children: vec![
                DropdownItem::default(),
                DropdownItem::default(),
                DropdownItem::default(),
                DropdownItem::default(),
            ],
            selected: 0,
            style: STYLE_DEFAULT,
            is_open: false,
            in_focus: false,
            depth: 0,
            fetched_text: None,
        }
    }
}

impl DropwdownTrait for Dropdown<'_> {
    fn get_children(&self) -> &Vec<DropdownItem> {
        &self.children
    }

    fn mut_get_children(&mut self) -> &mut Vec<DropdownItem> {
        &mut self.children
    }

    fn selected(&self) -> Option<usize> {
        match self.is_open() {
            true => Some(self.selected),
            false => None,
        }
    }

    fn open(&mut self) {
        self.alter_dropdown(DropdownAction::Open);
    }

    fn close(&mut self) {
        self.alter_dropdown(DropdownAction::Close);
    }

    fn is_open(&self) -> bool {
        self.is_open
    }
}

impl Widget for Dropdown<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut new_area = area;
        let mut max_width = 0;

        for item in self.children.iter() {
            let text: Text<'_> = item.text.clone().into();
            max_width = max_width.max(text.width() as u16);
        }

        match self.header.clone() {
            DropdownHeader::Search(mut input) => {
                input.block = input
                    .block
                    .clone()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Plain);
                if self.in_focus {
                    input.block = input
                        .block
                        .border_style(input.style.add_modifier(Modifier::REVERSED));
                }
                input.style = self.style;
                input.clone().render(area, buf);
            }
            DropdownHeader::Title(title) => {
                // -1 because the text starts after the left border (so subtracting 1 ignores the right border too)
                let width = (area.width - 1) as usize;
                let title = format!("{: ^width$}", title).to_string();
                let title: Text = title.into();
                let mut block = Block::default()
                    .style(self.style)
                    .borders(Borders::ALL)
                    .title("▼")
                    .title_position(Position::Bottom)
                    .title_alignment(Alignment::Center);
                if self.in_focus {
                    block = block.border_style(self.style.add_modifier(Modifier::REVERSED))
                }

                let title = Paragraph::new(title).block(block).style(self.style);
                title.render(area, buf);
            }
        };

        let window_resolution = (
            WINDOW_RESOLUTION
                .0
                .load(std::sync::atomic::Ordering::Acquire),
            WINDOW_RESOLUTION
                .1
                .load(std::sync::atomic::Ordering::Acquire),
        );
        let window_midpoint = window_resolution.0 / 2;
        let mut go_left: Option<bool> = None;
        let mut render_dropdown = |dropdown: &dyn DropwdownTrait, style: Style| {
            let selected = dropdown.selected();
            let dimensions = dropdown.dimensions();
            let (list_width, list_height) = (dimensions.0 as u16, dimensions.1 as u16);

            let mut list = match dropdown.to_list() {
                Some(list) => list
                    .style(style)
                    .highlight_style(style.add_modifier(Modifier::REVERSED)),
                None => return,
            };
            if self.in_focus {
                list = list.block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(style.add_modifier(Modifier::REVERSED)),
                )
            }

            let x = if go_left.is_none() {
                let midpoint = new_area.x + (list_width / 2);
                if midpoint > window_midpoint {
                    go_left = Some(true);
                } else {
                    go_left = Some(false);
                }
                new_area.left()
            } else if go_left.unwrap() {
                new_area.left() - list_width
            } else {
                new_area.right()
            };
            let y = new_area.bottom();
            let submenu_area = Rect::new(
                x,
                y,
                list_width,
                window_resolution.1.saturating_sub(y + 1).min(list_height)
            );

            new_area = Rect::new(
                submenu_area.x,
                submenu_area
                    .top()
                    .saturating_add(selected.unwrap_or(0).saturating_sub(1) as u16),
                submenu_area.width,
                selected.unwrap_or(0) as u16,
            );
            Clear::default().render(submenu_area, buf);
            StatefulWidget::render(
                list,
                submenu_area,
                buf,
                &mut ListState::default().with_offset(0).with_selected(selected),
            );
        };

        let menu: &dyn DropwdownTrait = &self as &dyn DropwdownTrait;
        match menu.is_open() & self.in_focus {
            true => {
                render_dropdown(menu, self.style);
                let mut children = menu.get_children().to_owned();
                let mut empty = children.is_empty();
                let mut children_to_render = vec![];

                while !empty {
                    // Setting empty as true here means the loop only continues if one of the children is open
                    empty = true;
                    for child in children.clone() {
                        if child.is_open() {
                            children = child.get_children().to_owned();
                            empty = child.children.is_empty();
                            children_to_render.push(child);
                            break;
                        }
                    }
                }
                for child in children_to_render.iter() {
                    render_dropdown(child as &dyn DropwdownTrait, self.style);
                }
            }
            false => (),
        }
    }
}

impl WidgetWithHints for Dropdown<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        vec![
            (
                Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
                String::from("Next item"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
                String::from("Previous item"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                String::from("Open item"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                String::from("Close item"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
                String::from("Click item"),
            ),
        ]
    }
}

impl FocusableWidget for Dropdown<'_> {
    fn make_focused(&mut self) {
        self.in_focus = true;
        match &mut self.header {
            DropdownHeader::Search(input) => input.in_focus = true,
            DropdownHeader::Title(_) => (),
        };
    }
}

impl SLSKWidget for Dropdown<'_> {}

impl SelectItem for Dropdown<'_> {
    fn get_index(&self) -> Option<usize> {
        self.selected()
    }

    fn set_index(&mut self, index: Option<usize>) {
        if index.is_some() {
            self.selected = index.unwrap();
        }
    }

    fn select_previous(&mut self) {
        self.alter_dropdown(DropdownAction::Previous);
    }

    fn select_next(&mut self) {
        self.alter_dropdown(DropdownAction::Next);
    }
}

impl EventHandler for Dropdown<'_> {
    fn handle_event(&mut self, evt: &Event) -> Option<tui_input::StateChanged> {
        if let Event::Key(key) = evt {
            if key.modifiers == KeyModifiers::NONE {
                match key.code {
                    KeyCode::Enter => self.open(),
                    KeyCode::Esc => self.close(),
                    KeyCode::Up => self.select_previous(),
                    KeyCode::Down => self.select_next(),
                    _ => (),
                }
            } else if key.modifiers == KeyModifiers::SHIFT {
                match key.code {
                    KeyCode::Enter => self.click(),
                    _ => (),
                }
            }
        }

        match &mut self.header {
            DropdownHeader::Search(input) => {
                input.handle_event(evt);
            }
            DropdownHeader::Title(_) => (),
        };
        None
    }
}
