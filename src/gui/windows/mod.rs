// TODO:

// File windows:
// SearchWindow
// DownloadWindow

// Social windows:
// ChatroomWindow
// MessageWindow

use crossterm::event::{Event, KeyEvent};
use ratatui::widgets::Widget;
use tokio::sync::broadcast::Sender;
use tui_input::backend::crossterm::EventHandler;

use crate::events::SLSKEvents;

use self::login::LoginWindow;

pub(crate) mod login;

/// A widget that has assosciated shortcut hints
pub(crate) trait WidgetWithHints: Widget {
    fn get_hints(&self) -> Vec<(KeyEvent, String)>;
}

pub(crate) trait Window: WidgetWithHints {
    fn get_title(&self) -> String;
    fn perform_action(&mut self, focus_index: u8, key: KeyEvent, write_queue: &Sender<SLSKEvents>);
    fn number_of_widgets(&self) -> u8;
    fn get_widget(&self, index: u8) -> &dyn WidgetWithHints;
    fn get_focused_index(&self) -> u8;
    fn set_focused_index(&mut self, index: u8);
}

impl Window for LoginWindow<'_> {
    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn perform_action(&mut self, focus_index: u8, key: KeyEvent, write_queue: &Sender<SLSKEvents>) {
        match focus_index {
            0 => self.username_input.handle_event(&Event::Key(key)),
            1 => self.password_input.handle_event(&Event::Key(key)),
            2 => {
                if key == self.login_button.key_event
                    && !self.username_input.input_string.is_empty()
                    && !self.password_input.input_string.is_empty()
                {
                    let username = self.username_input.input_string.clone();
                    let password = self.password_input.input_string.clone();
                    let _ = write_queue.send(SLSKEvents::TryLogin { username, password });
                };
                None
            }
            _ => unimplemented!("perform_action({focus_index}, {key:?})"),
        };
    }

    fn number_of_widgets(&self) -> u8 {
        3
    }

    fn get_widget(&self, index: u8) -> &dyn WidgetWithHints {
        match index {
            0 => &self.username_input,
            1 => &self.password_input,
            2 => &self.login_button,
            _ => unimplemented!(
                "There are only {} widgets, it's impossible to get the widget with index {index}",
                self.number_of_widgets()
            ),
        }
    }

    fn get_focused_index(&self) -> u8 {
        self.focus_index
    }

    fn set_focused_index(&mut self, index: u8) {
        self.focus_index = index;
    }
}
