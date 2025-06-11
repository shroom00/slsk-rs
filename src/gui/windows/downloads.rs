use std::{collections::HashMap, sync::Arc};

pub(crate) use crossterm::event::Event;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};
use tokio::sync::{broadcast::Sender, RwLock};
use tui_input::backend::crossterm::EventHandler;

use crate::{
    constants::{ByteSize, DownloadStatus, Percentage, Token},
    events::SLSKEvents,
    table::{ColumnData, TableItem, TableWidget},
};

use super::{FocusableWidget, SLSKWidget, WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct DownloadsWindow<'a> {
    title: String,
    focus_index: u8,
    downloads: TableWidget<'a>,
    /// Key: u32 token
    ///
    /// Values: DownloadStatus, Percentage
    pub(crate) download_state: HashMap<u32, (Arc<RwLock<DownloadStatus>>, Arc<RwLock<Percentage>>)>,
}

impl Default for DownloadsWindow<'_> {
    fn default() -> Self {
        Self {
            title: String::from(" Downloads "),
            downloads: TableWidget::new(
                vec![
                    String::from("User"),
                    String::from("Folder"),
                    String::from("Filename"),
                    String::from("Status"),
                    String::from("Progress"),
                    String::from("Filesize"),
                    // TODO: Speed, Time Elapsed, Time Left
                ],
                Vec::new(),
                None,
                Some(vec![
                    Constraint::Max(30), // username
                    Constraint::Fill(1), // folder
                    Constraint::Fill(2), // filename
                    Constraint::Max(18), // status
                    Constraint::Max(8),  // progress
                    Constraint::Max(10), // filesize
                ]),
            ),
            focus_index: 0,
            download_state: HashMap::new(),
        }
    }
}

impl Widget for DownloadsWindow<'_> {
    fn render(mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Fill(1)])
            .split(area);

        render_widgets!(
            SELF: self,
            BUFFER: buf,
            0 = (self.downloads )=> chunks[0],
        );
    }
}

impl WidgetWithHints for DownloadsWindow<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        self.get_widget(self.focus_index)
            .and_then(|w| Some(w.get_hints()))
            .unwrap_or_default()
    }
}

impl Window<'_> for DownloadsWindow<'_> {
    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn perform_action(&mut self, focus_index: u8, key: Event, _write_queue: &Sender<SLSKEvents>) {
        match focus_index {
            0 => self.downloads.handle_event(&key),
            _ => unimplemented!("perform_action({focus_index}, {key:?})"),
        };
    }

    fn number_of_widgets(&self) -> u8 {
        1
    }

    fn get_widget(&self, index: u8) -> Option<&dyn SLSKWidget> {
        match index {
            0 => Some(&self.downloads),
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

impl DownloadsWindow<'_> {
    fn add_item_helper(&mut self, item: TableItem, username: String, filesize: ByteSize) {
        let item_len = item.length(self.downloads.filter().as_deref().map(|f| f.as_str()));
        match self
            .downloads
            .items
            .iter_mut()
            .find(|root_item| &root_item.content[0].to_string() == &username)
        {
            Some(root_item) => {
                self.downloads.length += item_len;
                let size = TryInto::<ByteSize>::try_into(root_item.content.remove(5)).unwrap();
                root_item.content.insert(5, (size + filesize).into());
                root_item.children.push(item);
            }
            None => self.downloads.insert_item(
                TableItem::new(
                    vec![
                        username.into(),
                        ColumnData::Empty,
                        ColumnData::Empty,
                        DownloadStatus::Queued.into(),
                        Percentage(0).into(),
                        filesize.into(),
                    ],
                    vec![item],
                )
                .open(),
            ),
        }
    }

    pub(crate) fn add_file(
        &mut self,
        username: String,
        folder: String,
        filename: String,
        filesize: ByteSize,
        token: Token,
        status: Arc<RwLock<DownloadStatus>>,
        percentage: Arc<RwLock<Percentage>>,
    ) {
        let item = TableItem::new(
            vec![
                username.clone().into(),
                folder.into(),
                ColumnData::Empty,
                DownloadStatus::Queued.into(),
                Percentage(0).into(),
                filesize.into(),
            ],
            vec![TableItem::new(
                {
                    let status = ColumnData::DownloadStatus(status);
                    let percentage = ColumnData::Percentage(percentage);
                    self.download_state.insert(
                        token.0,
                        (
                            status.get_cell_data::<DownloadStatus>().unwrap().to_owned(),
                            percentage.get_cell_data::<Percentage>().unwrap().to_owned(),
                        ),
                    );
                    vec![
                        username.clone().into(),
                        ColumnData::Empty,
                        filename.into(),
                        status,
                        percentage,
                        filesize.into(),
                        token.into(),
                    ]
                },
                Vec::new(),
            )
            .open()],
        )
        .open();
        self.add_item_helper(item, username, filesize);
    }

    pub(crate) fn add_folder(
        &mut self,
        username: String,
        folder: String,
        files: Vec<(
            String,
            ByteSize,
            Token,
            Arc<RwLock<DownloadStatus>>,
            Arc<RwLock<Percentage>>,
        )>,
    ) {
        let total_filesize = ByteSize(files.iter().map(|(_, b, _, _, _)| b.0).sum());

        let item = TableItem::new(
            vec![
                username.clone().into(),
                folder.into(),
                ColumnData::Empty,
                DownloadStatus::Queued.into(),
                Percentage(0).into(),
                total_filesize.into(),
            ],
            files
                .into_iter()
                .map(|(filename, filesize, token, status, percentage)| {
                    let percentage = ColumnData::Percentage(percentage);
                    let status = ColumnData::DownloadStatus(status);
                    self.download_state.insert(
                        token.0,
                        (
                            status.get_cell_data::<DownloadStatus>().unwrap().to_owned(),
                            percentage.get_cell_data::<Percentage>().unwrap().to_owned(),
                        ),
                    );

                    TableItem::new(
                        vec![
                            username.clone().into(),
                            ColumnData::Empty,
                            filename.into(),
                            status,
                            percentage,
                            filesize.into(),
                            token.into(),
                        ],
                        Vec::new(),
                    )
                    .open()
                })
                .collect(),
        )
        .open();
        self.add_item_helper(item, username, total_filesize);
    }
}
