use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    constants::{ByteSize, ConnectionTypes, DownloadStatus, Percentage},
    messages::UserStats,
    FileSearchResponse,
};

#[rustfmt::skip]
#[derive(Clone, Debug)]
pub enum SLSKEvents {
    TryLogin { username: String, password: String },
    LoginResult { success: bool, reason: Option<String> },
    Quit,
    RoomList { rooms_and_num_of_users: Vec<(String, u32)> },
    /// Think of `private` like a boolean. 0 means public, anything else means private.
    JoinRoom { room: String, private: u32 },
    LeaveRoom { room: String },
    UpdateRoom { room: String, stats: Vec<(String, UserStats)> },
    ChatroomMessage { room: String, username: Option<String>, message: String },
    FileSearch { query: String, token: u32 },
    SearchResults ( FileSearchResponse ),
    GetInfo ( String ),
    Connect { username: String, token: u32, connection_type: ConnectionTypes},
    QueueMessage { token: u32, message_bytes: Vec<u8> },
    NewDownloads { username: String, folder: String, files: Vec<(String, ByteSize)>, from_all: bool },
    NewDownload { username: String, folder: String, filename: String, filesize: ByteSize },
    UpdateDownload { filename: String, status: Arc<RwLock<DownloadStatus>>, percentage: Arc<RwLock<Percentage>> },
    UpdateDownloads { files: Vec<(String, Arc<RwLock<DownloadStatus>>, Arc<RwLock<Percentage>>)>, from_all: bool }
}
