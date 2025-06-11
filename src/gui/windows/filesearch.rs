use std::{collections::HashMap, rc::Rc};

pub(crate) use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use rand::random;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};
use tokio::sync::broadcast::Sender;
use tui_input::backend::crossterm::EventHandler;

use crate::{
    constants::{ByteSize, ConnectionTypes, Token},
    events::SLSKEvents,
    gui::widgets::{
        dialog::Dialog,
        input::Input,
        table::{ColumnData, TableItem, TableWidget, ITEM_INTERACTED},
        tabs::{Tabs, TAB_REMOVED},
    },
    utils::default_results_table,
    FileSearchResponse, MessageTrait, QueueUpload, MAX_RESULTS,
};

use super::{FocusableWidget, SLSKWidget, WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct FileSearchWindow<'a> {
    pub(crate) title: String,
    pub(crate) focus_index: u8,
    pub(crate) search_tabs: Tabs<'a>,
    pub(crate) search_bar: Input<'a>,
    pub(crate) token_query_map: HashMap<String, u32>,
    pub(crate) results: HashMap<u32, (u32, TableWidget<'a>)>,
    pub(crate) dialog: Dialog<'a, (TableItem, Option<bool>)>,
}

impl FileSearchWindow<'_> {
    pub(crate) fn get_current_table(&self) -> Option<(u32, &TableWidget)> {
        if let Some(current_tab) = self.search_tabs.current_tab() {
            let token = self.token_query_map.get(current_tab).unwrap();
            if let Some((total, table)) = self.results.get(token) {
                return Some((*total, table));
            }
        };
        None
    }

    pub(crate) fn new_table(&mut self, token: u32, query: String) {
        let table = default_results_table();

        self.token_query_map.insert(query.clone(), token);
        self.search_tabs.add_tab(query);
        self.results.insert(token, (0, table));
    }

    pub(crate) fn add_results(&mut self, mut results: FileSearchResponse) {
        let token = results.token;
        if let Some((total, table)) = self.results.get_mut(&token) {
            let username = results.username;
            let avg_speed = ByteSize(results.avg_speed as u64);
            let queue_length = results.queue_length as usize;
            let header_text = vec![
                username.clone().into(),
                avg_speed.clone().into(),
                queue_length.into(),
            ];

            // Sort files by filename
            results
                .files
                .sort_by(|f1, f2| f1.filename.cmp(&f2.filename));

            let mut current_folder: Option<String> = None;
            let mut folder_rows: Vec<TableItem> = Vec::new();
            let mut rows: Vec<Vec<ColumnData>> = Vec::new();

            let folder_header = |current_folder: Option<String>| {
                vec![
                    username.clone().into(),        // username
                    avg_speed.into(),               // average speed
                    queue_length.into(),            // queue length
                    current_folder.unwrap().into(), // folder
                ]
            };
            for result in results.files {
                let (folder, filename) = match result.filename.rsplit_once('/') {
                    Some((folder, filename)) => (
                        {
                            let mut folder = folder.to_string();
                            folder.push('/');
                            folder
                        },
                        filename.to_string(),
                    ),
                    None => match result.filename.rsplit_once('\\') {
                        Some((folder, filename)) => (
                            {
                                let mut folder = folder.to_string();
                                folder.push('\\');
                                folder
                            },
                            filename.to_string(),
                        ),
                        None => (String::new(), result.filename),
                    },
                };

                if current_folder.is_some() && current_folder != Some(folder.clone()) {
                    let rows_for_folder = rows
                        .into_iter()
                        .map(|row| TableItem::new(ColumnData::from_vec(row), Vec::new()).opened())
                        .collect();
                    folder_rows.push(
                        TableItem::new(folder_header(current_folder), rows_for_folder).opened(),
                    );
                    rows = Vec::new(); // Reset rows for new folder
                }

                rows.push(vec![
                    username.clone().into(),           // username
                    avg_speed.into(),                  // average speed
                    queue_length.into(),               // queue length
                    ColumnData::Empty,                 // folder
                    filename.into(),                   // file name
                    ByteSize(result.file_size).into(), // file size
                    folder.clone().into(),
                ]);
                *total += 1;

                current_folder = Some(folder);
            }

            // Add remaining folder's rows (for the last folder in the loop)
            if !rows.is_empty() {
                let rows_for_folder = rows
                    .into_iter()
                    .map(|row| TableItem::new(ColumnData::from_vec(row), Vec::new()).opened())
                    .collect();

                folder_rows.push(
                    TableItem::new(
                        ColumnData::from_vec(folder_header(current_folder)),
                        rows_for_folder,
                    )
                    .opened(),
                );
            }

            // Add to results if there are any folder rows
            if !folder_rows.is_empty() {
                table.insert_item(TableItem::new(header_text, folder_rows).opened());
            }
        }
    }
}

impl Default for FileSearchWindow<'_> {
    fn default() -> Self {
        Self {
            title: String::from(" File Search "),
            focus_index: 0,
            search_tabs: Tabs::default().title(String::from("Search Results")),
            search_bar: Input::default().title("File Search".to_string()),
            results: HashMap::default(),
            token_query_map: HashMap::default(),
            dialog: Dialog::default().yes_no_funcs(
                Some(Rc::new(|_, (write_queue, (item, download_type))| {
                    let (is_all, is_folder) =
                        (download_type.is_none(), download_type.is_some_and(|b| b));
                    let username = item.content[0].to_string();

                    let queue_file = |file: &str, username: String, folder: &str| {
                        let token = random::<u32>();

                        let queue_upload = QueueUpload {
                            filename: format!("{folder}{file}"),
                        };
                        let message_bytes = QueueUpload::to_bytes(queue_upload);
                        write_queue
                            .send(SLSKEvents::QueueMessage {
                                token,
                                message_bytes,
                            })
                            .unwrap();

                        write_queue
                            .send(SLSKEvents::Connect {
                                username,
                                token,
                                connection_type: ConnectionTypes::PeerToPeer,
                            })
                            .unwrap();
                        token
                    };

                    if is_all | is_folder {
                        let folder_items = if is_all {
                            item.children.as_slice()
                        } else {
                            &[item]
                        };
                        for folder_item in folder_items {
                            let folder = folder_item.content[3].to_string();

                            write_queue
                                .send(SLSKEvents::NewDownloads {
                                    username: username.clone(),
                                    files: folder_item
                                        .children
                                        .iter()
                                        .map(|item| {
                                            let filename = item.content[4].to_string();
                                            let filesize =
                                                item.content[5].clone().try_into().unwrap();
                                            let token =
                                                queue_file(&filename, username.clone(), &folder);
                                            (filename, filesize, Token(token))
                                        })
                                        .collect(),
                                    folder,
                                    from_all: is_all,
                                })
                                .unwrap();
                        }
                    } else {
                        let filename = item.content[4].to_string();
                        let filesize = item.content[5].clone().try_into().unwrap();
                        let folder = item.content[6].to_string();
                        let username = username.to_string();
                        let token = Token(queue_file(&filename, username.clone(), &folder));
                        write_queue
                            .send(SLSKEvents::NewDownload {
                                username,
                                folder,
                                filename,
                                filesize,
                                token,
                            })
                            .unwrap();
                    };
                })),
                None,
            ),
        }
    }
}

impl Widget for FileSearchWindow<'_> {
    fn render<'a>(mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let chunks = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ],
        )
        .split(area);

        let mut table = match self.get_current_table() {
            Some((total, table)) => table.title(if total <= MAX_RESULTS {
                format!("{total} results")
            } else {
                format!("{MAX_RESULTS}+ results")
            }),
            None => default_results_table(),
        };
        render_widgets!(
            SELF: self,
            BUFFER: buf,
            0 = (self.search_bar) => chunks[0],
            1 = (self.search_tabs) => chunks[1],
            2 = (table) => chunks[2],
        );

        if self.dialog.visible {
            self.dialog.render(area, buf);
        }
    }
}

impl WidgetWithHints for FileSearchWindow<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        if let Some(widget) = self.get_widget(self.focus_index) {
            widget.get_hints()
        } else {
            Vec::new()
        }
    }
}

impl Window<'_> for FileSearchWindow<'_> {
    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn perform_action(&mut self, focus_index: u8, event: Event, write_queue: &Sender<SLSKEvents>) {
        match focus_index {
            0 => {
                if event == Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
                    let query = self.search_bar.input_string.clone();
                    if !self.search_tabs.tabs.contains(&query) {
                        let _ = write_queue.send(SLSKEvents::FileSearch {
                            query: query.clone(),
                            token: {
                                loop {
                                    let token = random();
                                    if !self.results.contains_key(&token) {
                                        self.new_table(token, query);
                                        break token;
                                    }
                                }
                            },
                        });
                    }
                } else {
                    self.search_bar.handle_event(&event);
                }
            }
            1 => {
                if let Some(selected_tab) = self.search_tabs.selected_tab().cloned() {
                    if self.search_tabs.handle_event(&event) == Some(TAB_REMOVED) {
                        let token = self.token_query_map.remove(&selected_tab).unwrap();
                        self.results.remove(&token).unwrap();
                    };
                };
            }
            2 => {
                if let Some(current_tab) = self.search_tabs.selected_tab() {
                    let token = self.token_query_map.get(current_tab).unwrap();
                    let table = &mut self.results.get_mut(token).unwrap().1;
                    if table.handle_event(&event) == Some(ITEM_INTERACTED) {
                        if let Some(item) = table.current_row() {
                            let username = &item.content[0].to_string();
                            let question;
                            if item.content.len() < 4 {
                                question = format!("Download all files from {username}?");
                                self.dialog.state = Some((item.clone(), None));
                            } else {
                                let (is_folder, download_fp) = if item.content.len() > 4 {
                                    (Some(false), &item.content[4])
                                } else {
                                    (Some(true), &item.content[3])
                                };
                                question = format!(
                                    "Download {} from {}?",
                                    download_fp.to_string(),
                                    username.to_string()
                                );
                                self.dialog.state = Some((item.clone(), is_folder));
                            }

                            self.dialog.set_question(question);

                            self.dialog.show();
                        };
                    }
                }
            }
            _ => unimplemented!("perform_action({focus_index}, {event:?})"),
        };
    }

    fn number_of_widgets(&self) -> u8 {
        if self.dialog.visible {
            1
        } else {
            3
        }
    }

    fn get_widget(&self, index: u8) -> Option<&dyn SLSKWidget> {
        if self.dialog.visible {
            Some(&self.dialog)
        } else {
            match index {
                0 => Some(&self.search_bar),
                1 => Some(&self.search_tabs),
                2 => match self.get_current_table() {
                    Some((_, table)) => Some(table as &dyn SLSKWidget),
                    None => None,
                },
                _ => unimplemented!(
                "There are only {} widgets, it's impossible to get the widget with index {index}",
                self.number_of_widgets()
            ),
            }
        }
    }

    fn get_focused_index(&self) -> u8 {
        self.focus_index
    }

    fn set_focused_index(&mut self, index: u8) {
        self.focus_index = index;
    }
}
