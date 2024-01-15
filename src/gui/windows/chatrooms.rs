use byte_unit::Byte;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ordered_hash_map::OrderedHashMap;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::{Row, Widget},
};
use tokio::sync::broadcast::Sender;
use tui_input::{backend::crossterm::EventHandler, StateChanged};

use crate::{
    events::SLSKEvents,
    gui::widgets::{
        chatroom::ChatroomState,
        dropdown::{Dropdown, DropdownHeader},
        input::Input,
        list::List,
        table::Table,
        tabs::Tabs,
    },
    messages::UserStats,
    utils::{num_as_bytes, num_as_str},
};

use super::{FocusableWidget, SLSKWidget, WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct ChatroomsWindow<'a> {
    pub(crate) title: String,
    pub(crate) chatrooms: OrderedHashMap<String, ChatroomState>,
    pub(crate) room_name_tabs: Tabs<'a>,
    pub(crate) room_sidebar: Table<'a>,
    pub(crate) rooms_dropdown: Dropdown<'a>,
    pub(crate) message_input: Input<'a>,
    pub(crate) sidebar_scroll_offset: usize,
    pub(crate) sidebar_selected: Option<usize>,
    pub(crate) focus_index: u8,
}

impl ChatroomsWindow<'_> {
    pub(crate) fn update_sidebar(&mut self) {
        let mut users = self
            .chatrooms
            .get(&self.room_name_tabs.titles[self.room_name_tabs.current])
            .unwrap()
            .users
            .iter()
            .collect::<Vec<(&String, &UserStats)>>();
        users.sort_by_key(|(username, _)| username.to_ascii_lowercase());

        self.room_sidebar.rows = users
            .iter()
            .map(|(user, stats)| {
                Row::new(vec![
                    user.to_string(),
                    num_as_bytes(stats.avg_speeds as u64),
                    num_as_str(stats.num_of_files),
                ])
            })
            .collect();
    }

    pub(crate) fn get_current_chatroom_state(&self) -> Option<&ChatroomState> {
        match self.room_name_tabs.current_title() {
            Some(title) => self.chatrooms.get(title),
            None => None,
        }
    }

    pub(crate) fn get_specific_chatroom_state(&self, room: &str) -> Option<&ChatroomState> {
        self.chatrooms.get(room)
    }

    pub(crate) fn get_mut_current_chatroom_state(&mut self) -> Option<&mut ChatroomState> {
        match self.room_name_tabs.current_title() {
            Some(title) => self.chatrooms.get_mut(title),
            None => None,
        }
    }

    pub(crate) fn get_mut_specific_chatroom_state(
        &mut self,
        room: &str,
    ) -> Option<&mut ChatroomState> {
        self.chatrooms.get_mut(room)
    }
}

impl Default for ChatroomsWindow<'_> {
    fn default() -> Self {
        Self {
            title: String::from(" Chatrooms "),
            chatrooms: OrderedHashMap::new(),
            room_name_tabs: {
                let tabs = Tabs::default().titles(vec![]);
                tabs.block(tabs.block.clone().title("Rooms"))
            },
            room_sidebar: Table::new(Some(Row::new(["Users", "Upload Speed", "Files"])), vec![])
                .widths(&[
                    Constraint::Length(30),
                    Constraint::Length(12),
                    Constraint::Length(15),
                ]),
            rooms_dropdown: Dropdown::new(DropdownHeader::Title(&"All Rooms"), vec![]),
            message_input: Input::default().title(String::from("Message Input")),
            sidebar_scroll_offset: 0,
            sidebar_selected: None,
            focus_index: 0,
        }
    }
}

impl<'a> Widget for ChatroomsWindow<'a> {
    fn render(mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let main_area = Layout::new()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                // 30 for usernames (they can't be longer)
                // 12 for upload speed
                // 15 for number of files
                // 000,000,000,000
                // (allows for billions of files because who has that many files?)
                Constraint::Length(57),
            ])
            .split(area);
        let message_area = Layout::new()
            .direction(Direction::Vertical)
            .constraints([
                // Room tabs
                Constraint::Length(3),
                // Message area
                Constraint::Min(0),
                // Input
                Constraint::Length(3),
            ])
            .split(main_area[0]);

        let above_room_area = Layout::new()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(13)])
            .split(message_area[0]);

        let messages = List::new(match self.get_current_chatroom_state() {
            Some(state) => state.messages.clone(),
            None => vec![],
        });
        messages.render(message_area[1], buf);

        render_widgets!(
            SELF: self,
            BUFFER: buf,
            0 = (self.room_name_tabs) => above_room_area[0],
            1 = (self.rooms_dropdown) => above_room_area[1],
            2 = (self.message_input) => message_area[2],
            3 = (self.room_sidebar) => main_area[1],
        );
    }
}

impl WidgetWithHints for ChatroomsWindow<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        self.get_widget(self.focus_index).get_hints()
    }
}

impl Window<'_> for ChatroomsWindow<'_> {
    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn perform_action(&mut self, focus_index: u8, event: Event, write_queue: &Sender<SLSKEvents>) {
        match focus_index {
            0 => {
                if self.room_name_tabs.handle_event(&event)
                    == Some(StateChanged {
                        value: true,
                        cursor: true,
                    })
                {
                    self.update_sidebar();
                };
                None
            }
            1 => {
                let out = self.rooms_dropdown.handle_event(&event);
                if self.rooms_dropdown.fetched_text.is_some() {
                    let room = self.rooms_dropdown.fetched_text.as_ref().unwrap();
                    if !self.room_name_tabs.titles.contains(&room) {
                        self.room_name_tabs.add_title(room.to_string());
                        let _ = write_queue.send(SLSKEvents::JoinRoom {
                            room: room.to_string(),
                            private: 0,
                        });
                    }
                    self.rooms_dropdown.fetched_text = None;
                };
                out
            }
            2 => {
                if event == Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
                    match self.room_name_tabs.current_title() {
                        Some(room) => {
                            let _ = write_queue.send(SLSKEvents::ChatroomMessage {
                                room: room.to_string(),
                                username: None,
                                message: self.message_input.input_string.clone(),
                            });
                            self.message_input.clear();
                            None
                        }
                        None => None,
                    }
                } else {
                    self.message_input.handle_event(&event)
                }
            }
            3 => self.room_sidebar.handle_event(&event),
            _ => unimplemented!("perform_action({focus_index}, {event:?})"),
        };
    }

    fn number_of_widgets(&self) -> u8 {
        4
    }

    fn get_widget(&self, index: u8) -> &dyn SLSKWidget {
        match index {
            0 => &self.room_name_tabs,
            1 => &self.rooms_dropdown,
            2 => &self.message_input,
            3 => &self.room_sidebar,
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
