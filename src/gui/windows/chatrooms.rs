use std::mem;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ordered_hash_map::OrderedHashMap;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};
use tokio::sync::broadcast::Sender;
use tui_input::backend::crossterm::EventHandler;

use crate::{
    constants::ByteSize,
    events::SLSKEvents,
    gui::widgets::{
        chatrooms::ChatroomState,
        dropdown::{Dropdown, DropdownHeader},
        input::Input,
        list::List,
        table::{TableItem, TableWidget},
        tabs::{Tabs, TAB_CHANGED, TAB_REMOVED},
    },
    messages::UserStats,
};

use super::{FocusableWidget, SLSKWidget, WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct ChatroomsWindow<'a> {
    pub(crate) title: String,
    pub(crate) chatrooms: OrderedHashMap<String, (ChatroomState, List<'a>)>,
    pub(crate) room_name_tabs: Tabs<'a>,
    pub(crate) room_sidebar: TableWidget<'a>,
    pub(crate) rooms_dropdown: Dropdown<'a>,
    pub(crate) message_input: Input<'a>,
    pub(crate) focus_index: u8,
}

impl ChatroomsWindow<'_> {
    pub(crate) fn update_sidebar(&mut self) {
        if !self.room_name_tabs.tabs.is_empty() {
            let mut users = self
                .chatrooms
                .get(&self.room_name_tabs.tabs[self.room_name_tabs.current])
                .unwrap()
                .0
                .users
                .iter()
                .collect::<Vec<(&String, &UserStats)>>();
            users.sort_by_key(|(username, _)| username.to_ascii_lowercase());

            self.room_sidebar.items = users
                .iter()
                .map(|(user, stats)| {
                    TableItem::new(
                        vec![
                            user.to_string().into(),
                            ByteSize(stats.avg_speeds as u64).into(),
                            (stats.num_of_files as usize).into(),
                        ],
                        Vec::new(),
                    )
                })
                .collect()
        } else {
            self.room_sidebar.items = Vec::new();
        }
    }

    pub(crate) fn get_current_chatroom_state(&self) -> Option<&ChatroomState> {
        match self.room_name_tabs.current_tab() {
            Some(tab) => match self.chatrooms.get(tab) {
                Some((state, _)) => Some(state),
                None => None,
            },
            None => None,
        }
    }

    pub(crate) fn get_mut_specific_chatroom_state(
        &mut self,
        room: &str,
    ) -> Option<&mut ChatroomState> {
        match self.chatrooms.get_mut(room) {
            Some((state, _)) => Some(state),
            None => None,
        }
    }
}

impl Default for ChatroomsWindow<'_> {
    fn default() -> Self {
        Self {
            title: String::from(" Chatrooms "),
            chatrooms: OrderedHashMap::new(),
            room_name_tabs: Tabs::default().title(String::from("Rooms")),
            room_sidebar: TableWidget::new(
                vec![
                    String::from("User"),
                    String::from("Upload Speed"),
                    String::from("Files"),
                ],
                Vec::new(),
                None,
                Some(vec![
                    Constraint::Length(30),
                    Constraint::Length(12),
                    Constraint::Length(15),
                ]),
            ),
            rooms_dropdown: Dropdown::new(DropdownHeader::Title("All Rooms"), Vec::new()),
            message_input: Input::default().title(String::from("Message Input")),
            focus_index: 0,
        }
    }
}

impl<'a> Widget for ChatroomsWindow<'a> {
    fn render(mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let main_area = Layout::new(
            Direction::Horizontal,
            [
                Constraint::Min(0),
                // 30 for usernames (they can't be longer)
                // 12 for upload speed
                // 15 for number of files
                // 000,000,000,000
                // (allows for billions of files because who has that many files?)
                Constraint::Length(57),
            ],
        )
        .split(area);
        let message_area = Layout::new(
            Direction::Vertical,
            [
                // Room tabs
                Constraint::Length(3),
                // Message area
                Constraint::Min(0),
                // Input
                Constraint::Length(3),
            ],
        )
        .split(main_area[0]);

        let above_room_area = Layout::new(
            Direction::Horizontal,
            [Constraint::Min(0), Constraint::Length(13)],
        )
        .split(message_area[0]);

        let message_len: Option<usize>;
        let mut messages = List::new(match self.get_current_chatroom_state() {
            Some(state) => {
                message_len = Some(state.messages.len().saturating_sub(1));
                state.messages.clone()
            }
            None => {
                message_len = None;
                Vec::new()
            }
        });
        messages.state.select(message_len);
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
        if let Some(widget) = self.get_widget(self.focus_index) {
            widget.get_hints()
        } else {
            Vec::new()
        }
    }
}

impl Window<'_> for ChatroomsWindow<'_> {
    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn perform_action(&mut self, focus_index: u8, event: Event, write_queue: &Sender<SLSKEvents>) {
        match focus_index {
            0 => {
                let result = self.room_name_tabs.handle_event(&event);
                if result == Some(TAB_CHANGED) {
                    self.update_sidebar();
                } else if result == Some(TAB_REMOVED) {
                    let room = mem::replace(&mut self.room_name_tabs.removed_tab, None);
                    let _ = write_queue.send(SLSKEvents::LeaveRoom {
                        room: room.unwrap(),
                    });
                    self.room_name_tabs.removed_tab = None;
                    self.update_sidebar();
                };
                None
            }
            1 => {
                let out = self.rooms_dropdown.handle_event(&event);
                if self.rooms_dropdown.fetched_text.is_some() {
                    let room = self.rooms_dropdown.fetched_text.as_ref().unwrap();
                    if !self.room_name_tabs.tabs.contains(&room) {
                        self.room_name_tabs.add_tab(room.to_string());
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
                    match self.room_name_tabs.current_tab() {
                        Some(room) => {
                            let _ = write_queue.send(SLSKEvents::ChatroomMessage {
                                room: room.to_string(),
                                username: None,
                                // we don't need to handle password values, because message_input will never be a password input
                                message: self.message_input.input.value().to_string(),
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

    fn get_widget(&self, index: u8) -> Option<&dyn SLSKWidget> {
        match index {
            0 => Some(&self.room_name_tabs),
            1 => Some(&self.rooms_dropdown),
            2 => Some(&self.message_input),
            3 => Some(&self.room_sidebar),
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
