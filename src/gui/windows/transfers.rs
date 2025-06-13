use std::sync::Arc;

pub(crate) use crossterm::event::Event;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};
use tokio::sync::{broadcast::Sender, RwLock};
use tui_input::backend::crossterm::EventHandler;

use crate::{
    constants::{ByteSize, DownloadStatus, Percentage}, events::SLSKEvents, table::{ColumnData, TableItem, TableWidget}
};

use super::{FocusableWidget, SLSKWidget, WidgetWithHints, Window};

#[derive(Clone)]
pub(crate) struct TransfersWindow<'a> {
    title: String,
    focus_index: u8,
    downloads: TableWidget<'a>,
}

impl Default for TransfersWindow<'_> {
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
        }
    }
}

impl Widget for TransfersWindow<'_> {
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

impl WidgetWithHints for TransfersWindow<'_> {
    fn get_hints(&self) -> Vec<(Event, String)> {
        self.get_widget(self.focus_index)
            .and_then(|w| Some(w.get_hints()))
            .unwrap_or_default()
    }
}

impl Window<'_> for TransfersWindow<'_> {
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

impl TransfersWindow<'_> {
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
                // Updating referenced statuses
                root_item.content[3] += item.content[3].clone();
                // Updating referenced percentages
                root_item.content[4] += item.content[4].clone();
                // Updating filesize
                root_item.content[5] += item.content[5].clone();
                root_item.children.push(item);
            }
            None => self.downloads.insert_item(
                TableItem::new(
                    vec![
                        username.into(),
                        ColumnData::Empty,
                        ColumnData::Empty,
                        item.content[3].clone(),
                        item.content[4].clone(),
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
        status: Arc<RwLock<DownloadStatus>>,
        percentage: Arc<RwLock<Percentage>>,
    ) {
        let item = TableItem::new(
            vec![
                username.clone().into(),
                folder.into(),
                ColumnData::Empty,
                ColumnData::DownloadStatus(Arc::clone(&status)),
                ColumnData::Percentages(vec![(Arc::clone(&percentage), filesize.0)]),
                filesize.into(),
            ],
            vec![TableItem::new(
                {
                    let percentage = ColumnData::Percentage(percentage);
                    vec![
                        username.clone().into(),
                        ColumnData::Empty,
                        filename.into(),
                        ColumnData::DownloadStatus(status),
                        percentage,
                        filesize.into(),
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
            Arc<RwLock<DownloadStatus>>,
            Arc<RwLock<Percentage>>,
        )>,
    ) {
        let total_filesize = ByteSize(files.iter().map(|(_, b, _, _)| b.0).sum());

        let mut download_statuses = Vec::with_capacity(files.len());
        let mut percentages = Vec::with_capacity(files.len());

        let children = files
            .into_iter()
            .map(|(filename, filesize, status, percentage)| {
                download_statuses.push(Arc::clone(&status));
                percentages.push((Arc::clone(&percentage), filesize.0));

                TableItem::new(
                    vec![
                        username.clone().into(),
                        ColumnData::Empty,
                        filename.into(),
                        ColumnData::DownloadStatus(status),
                        ColumnData::Percentage(percentage),
                        filesize.into(),
                    ],
                    Vec::new(),
                )
                .open()
            })
            .collect();

        let folder = TableItem::new(
            vec![
                username.clone().into(),
                folder.into(),
                ColumnData::Empty,
                ColumnData::DownloadStatuses(download_statuses),
                ColumnData::Percentages(percentages),
                total_filesize.into(),
            ],
            children,
        )
        .open();
        self.add_item_helper(folder, username, total_filesize);
    }
}
