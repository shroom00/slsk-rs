// TODO:

// File windows:
// UploadWindow

// Social windows:
// MessageWindow

use crossterm::event::Event;
use ratatui::widgets::Widget;
use tokio::sync::broadcast::Sender;

use crate::events::SLSKEvents;

pub(crate) mod chatrooms;
pub(crate) mod filesearch;
pub(crate) mod login;
pub(crate) mod downloads;

/// A widget that has assosciated shortcut hints
pub(crate) trait WidgetWithHints: Widget {
    fn get_hints(&self) -> Vec<(Event, String)>;
}

pub(crate) trait FocusableWidget: Widget {
    /// Adjusts a Widget's styles etc. to make the Widget appear focused
    fn make_focused(&mut self);
}

pub(crate) trait SLSKWidget: WidgetWithHints + FocusableWidget {}

pub(crate) trait Window<'a>: WidgetWithHints {
    fn get_title(&self) -> String;
    fn perform_action<'b>(
        &'b mut self,
        focus_index: u8,
        event: Event,
        write_queue: &'b Sender<SLSKEvents>,
    );
    fn number_of_widgets(&self) -> u8;
    fn get_widget(&self, index: u8) -> Option<&dyn SLSKWidget>;
    fn get_focused_index(&self) -> u8;
    fn set_focused_index(&mut self, index: u8);
}
