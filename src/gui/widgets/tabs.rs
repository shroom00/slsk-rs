use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    widgets::{Block, Borders, Widget},
};
use tui_input::{backend::crossterm::EventHandler, StateChanged};

use crate::{
    gui::windows::{FocusableWidget, SLSKWidget, WidgetWithHints},
    styles::STYLE_DEFAULT,
    utils::vec_string_to_tabs,
};

use super::SelectItem;

pub(crate) const TAB_CHANGED: StateChanged = StateChanged {
    value: true,
    cursor: true,
};
pub(crate) const TAB_REMOVED: StateChanged = StateChanged {
    value: false,
    cursor: true,
};

#[derive(Clone)]
pub(crate) struct Tabs<'a> {
    pub(crate) style: Style,
    pub(crate) block: Block<'a>,
    pub(crate) tabs: Vec<String>,
    pub(crate) removed_tab: Option<String>,
    pub(crate) selected: usize,
    pub(crate) current: usize,
    pub(crate) in_focus: bool,
}

impl Default for Tabs<'_> {
    fn default() -> Self {
        Self {
            style: STYLE_DEFAULT,
            block: Block::default().style(STYLE_DEFAULT).borders(Borders::ALL),
            tabs: Default::default(),
            removed_tab: None,
            selected: Default::default(),
            current: Default::default(),
            in_focus: false,
        }
    }
}

#[allow(dead_code)]
impl<'a> Tabs<'a> {
    pub(crate) fn style(&self, style: Style) -> Self {
        let mut new = self.clone();
        new.style = style;
        new
    }

    pub(crate) fn title(&self, title: String) -> Self {
        let mut new = self.clone();
        new.block = new.block.title(title);
        new
    }

    pub(crate) fn block(&self, block: Block<'a>) -> Self {
        let mut new = self.clone();
        new.block = block;
        new
    }

    pub(crate) fn tabs(&self, tabs: Vec<String>) -> Self {
        let mut new = self.clone();
        new.tabs = tabs;
        new
    }

    pub(crate) fn remove_selected_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.tabs.remove(self.selected);
            if (self.selected != 0) && (self.selected >= self.tabs.len()) {
                self.selected -= 1;
            }
        }
    }

    pub(crate) fn add_tab(&mut self, tab: String) {
        self.tabs.push(tab);
    }

    pub(crate) fn selected_tab(&self) -> Option<&String> {
        if !self.tabs.is_empty() {
            Some(&self.tabs[self.selected])
        } else {
            None
        }
    }

    pub(crate) fn current_tab(&self) -> Option<&String> {
        if !self.tabs.is_empty() {
            Some(&self.tabs[self.current])
        } else {
            None
        }
    }
}

impl Widget for Tabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let tabs = vec_string_to_tabs(
            self.tabs,
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
            (
                Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
                String::from("Close tab"),
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
        if self.tabs.len() > 0 {
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
        if self.tabs.len() > 0 {
            let mut index = self.get_index();
            index = match index {
                Some(n) => {
                    if n == self.tabs.len() - 1 {
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
    fn handle_event(&mut self, evt: &Event) -> Option<StateChanged> {
        if let Event::Key(key) = evt {
            if key.modifiers == KeyModifiers::NONE {
                if key.code == KeyCode::Left {
                    self.select_previous();
                } else if key.code == KeyCode::Right {
                    self.select_next();
                } else if key.code == KeyCode::Enter {
                    self.current = self.selected;
                    return Some(TAB_CHANGED);
                } else if key.code == KeyCode::Backspace {
                    self.remove_selected_tab();
                }
            }
        }
        None
    }
}
