use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, Tabs as TuiTabs, Widget},
};
use tui_input::{backend::crossterm::EventHandler, StateChanged};

use crate::{
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT,
    utils::vec_string_to_tabs,
};

use super::SelectItem;

#[derive(Clone)]
pub(crate) struct Tabs<'a> {
    pub(crate) style: Style,
    pub(crate) block: Block<'a>,
    pub(crate) titles: Vec<String>,
    pub(crate) selected: usize,
    pub(crate) current: usize,
    pub(crate) in_focus: bool,
}

impl Default for Tabs<'_> {
    fn default() -> Self {
        Self {
            style: STYLE_DEFAULT,
            block: Block::default().style(STYLE_DEFAULT).borders(Borders::ALL),
            titles: Default::default(),
            selected: Default::default(),
            current: Default::default(),
            in_focus: false,
        }
    }
}

impl<'a> Tabs<'a> {
    pub(crate) fn style(&self, style: Style) -> Self {
        let mut new = self.clone();
        new.style = style;
        new
    }

    pub(crate) fn block(&self, block: Block<'a>) -> Self {
        let mut new = self.clone();
        new.block = block;
        new
    }

    pub(crate) fn titles(&self, titles: Vec<String>) -> Self {
        let mut new = self.clone();
        new.titles = titles;
        new
    }

    pub(crate) fn remove_title(&mut self, title: String) {
        if let Some(pos) = self
            .titles
            .iter()
            .position(|existing_title: &String| *existing_title == title)
        {
            self.titles.remove(pos);
        }
    }

    pub(crate) fn add_title(&mut self, title: String) {
        self.titles.push(title);
    }

    pub(crate) fn selected_title(&self) -> Option<&String> {
        if !self.titles.is_empty() {
            Some(&self.titles[self.selected])
        } else {
            None
        }
    }

    pub(crate) fn current_title(&self) -> Option<&String> {
        if !self.titles.is_empty() {
            Some(&self.titles[self.current])
        } else {
            None
        }
    }
}

impl Widget for Tabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let tabs = vec_string_to_tabs(
            self.titles,
            self.style,
            if self.in_focus {
                self.style.reversed()
            } else {
                self.style
            },
            self.selected,
            self.current,
        )
        .block(self.block);
        tabs.render(area, buf);
    }
}

impl WidgetWithHints for Tabs<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        vec![
            (
                Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
                String::from("Move to previous tab"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
                String::from("Move to next tab"),
            ),
            (
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                String::from("Select highlighted tab"),
            ),
        ]
    }
}

impl FocusableWidget for Tabs<'_> {
    fn make_focused(&mut self) {
        self.block = self.block.clone().border_style(self.style.reversed());
        self.in_focus = true;
    }
}

impl SLSKWidget for Tabs<'_> {}

impl SelectItem for Tabs<'_> {
    fn get_index(&self) -> Option<usize> {
        Some(self.selected)
    }

    fn set_index(&mut self, index: Option<usize>) {
        match index {
            Some(i) => self.selected = i,
            None => (),
        }
    }

    fn select_previous(&mut self) {
        if self.titles.len() > 0 {
            let mut index = self.get_index();
            index = match index {
                Some(n) => {
                    if n != 0 {
                        Some(n - 1)
                    } else {
                        Some(n)
                    }
                }
                None => unimplemented!(),
            };
            self.set_index(index);
        }
    }

    fn select_next(&mut self) {
        if self.titles.len() > 0 {
            let mut index = self.get_index();
            index = match index {
                Some(n) => {
                    if n == self.titles.len() - 1 {
                        Some(n)
                    } else {
                        Some(n + 1)
                    }
                }
                None => unimplemented!(),
            };
            self.set_index(index);
        }
    }
}

impl EventHandler for Tabs<'_> {
    /// Returns `Some(StateChanged{ value: true, cursor: true })` when the tab is changed.
    /// (Enter is pressed)
    fn handle_event(&mut self, evt: &Event) -> Option<tui_input::StateChanged> {
        if let Event::Key(key) = evt {
            if key.modifiers == KeyModifiers::NONE {
                if key.code == KeyCode::Left {
                    self.select_previous();
                } else if key.code == KeyCode::Right {
                    self.select_next();
                } else if key.code == KeyCode::Enter {
                    self.current = self.selected;
                    return Some(StateChanged {
                        value: true,
                        cursor: true,
                    });
                }
            }
        }
        None
    }
}
