use std::io::{Read, Write};

use flate2::{read::ZlibDecoder, write::ZlibEncoder};

use super::{MessageTrait, MessageType};
use crate::{
    constants::TransferDirections,
    packing::{PackToBytes, UnpackFromBytes},
};

define_message_to_send_and_receive!(GetSharedFileList {});
impl_message_trait!(
    GetSharedFileList < GetSharedFileList,
    GetSharedFileList > (MessageType::Peer(4))
);

#[derive(Debug, Clone)]
pub enum FileAttribute {
    Bitrate(u32),
    Duration(u32),
    VBR(bool),
    Encoder(u32),
    SampleRate(u32),
    BitDepth(u32),
}

impl FileAttribute {
    pub fn from_parts(
        bitrate: Option<u32>,
        duration: Option<u32>,
        vbr: Option<bool>,
        sample_rate: Option<u32>,
        bit_depth: Option<u32>,
    ) -> Vec<Self> {
        [
            bitrate.map(|bitrate| FileAttribute::Bitrate(bitrate)),
            duration.map(|duration| FileAttribute::Duration(duration)),
            vbr.map(|vbr| FileAttribute::VBR(vbr)),
            sample_rate.map(|sample_rate| FileAttribute::SampleRate(sample_rate)),
            bit_depth.map(|bit_depth| FileAttribute::BitDepth(bit_depth)),
        ]
        .into_iter()
        .filter_map(|attr| attr)
        .collect::<Vec<FileAttribute>>()
    }
}

impl PackToBytes for FileAttribute {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let (code, value) = match *self {
            Self::Bitrate(bitrate) => (0u32, bitrate),
            Self::Duration(duration) => (1, duration),
            Self::VBR(vbr) => (2, vbr as u32),
            Self::Encoder(encoder) => (3, encoder),
            Self::SampleRate(sample_rate) => (4, sample_rate),
            Self::BitDepth(depth) => (5, depth),
        };
        let mut bytes = code.pack_to_bytes();
        bytes.extend(value.pack_to_bytes());
        bytes
    }
}

impl UnpackFromBytes for FileAttribute {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let code = <u32>::unpack_from_bytes(bytes)?;
        let value = <u32>::unpack_from_bytes(bytes)?;
        match code {
            0 => Some(Self::Bitrate(value)),
            1 => Some(Self::Duration(value)),
            2 => Some(Self::VBR(value == 1)),
            3 => Some(Self::Encoder(value)),
            4 => Some(Self::SampleRate(value)),
            5 => Some(Self::BitDepth(value)),
            _ => None,
        }
    }
}

define_message_to_send_and_receive!(File {
    code: u8,
    filename: String,
    file_size: u64,
    extension: String,
    attributes: Vec<FileAttribute>,
});
define_message_to_send_and_receive!(Directory {
    path: String,
    files: Vec<File>,
});

define_message_to_send_and_receive!(SharedFileListRequest {});
impl_message_trait!(
    SharedFileListRequest < SharedFileListRequest,
    SharedFileListRequest > (MessageType::Peer(4))
);

#[derive(Debug, Clone)]
pub struct SharedFileListResponse {
    pub directories: Vec<Directory>,
    pub _unknown_0: u32,
    pub priv_directories: Vec<Directory>,
}

impl PackToBytes for SharedFileListResponse {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.directories.pack_to_bytes();
        bytes.extend(self._unknown_0.pack_to_bytes());
        bytes.extend(self.priv_directories.pack_to_bytes());

        let mut writer = ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        let _ = writer.write_all(&bytes);
        writer.finish().unwrap_or_default()
    }
}

impl UnpackFromBytes for SharedFileListResponse {
    fn unpack_from_bytes(buf: &mut Vec<u8>) -> Option<Self> {
        let mut bytes = Vec::new();
        ZlibDecoder::new(buf.as_slice())
            .read_to_end(&mut bytes)
            .ok()?;
        let directories = <Vec<Directory>>::unpack_from_bytes(&mut bytes)?;
        let _unknown_0 = <u32>::unpack_from_bytes(&mut bytes)?;
        let priv_directories = <Vec<Directory>>::unpack_from_bytes(&mut bytes)?;

        Some(Self {
            directories,
            _unknown_0,
            priv_directories,
        })
    }
}

impl_message_trait!(
    SharedFileListResponse < SharedFileListResponse,
    SharedFileListResponse > (MessageType::Peer(5))
);

#[derive(Debug, Clone)]
pub struct FileSearchResponse {
    pub username: String,
    pub token: u32,
    pub files: Vec<File>,
    pub slot_free: bool,
    pub avg_speed: u32,
    pub queue_length: u32,
    pub unknown_0: u32,
    pub private_files: Option<Vec<File>>,
}

impl PackToBytes for FileSearchResponse {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.username.pack_to_bytes();
        bytes.extend(self.token.pack_to_bytes());
        bytes.extend(self.files.pack_to_bytes());
        bytes.extend(self.slot_free.pack_to_bytes());
        bytes.extend(self.avg_speed.pack_to_bytes());
        bytes.extend(self.queue_length.pack_to_bytes());
        bytes.extend(self.unknown_0.pack_to_bytes());
        if let Some(private_files) = &self.private_files {
            bytes.extend(private_files.pack_to_bytes());
        };

        let mut writer = ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        let _ = writer.write_all(&bytes);
        writer.finish().unwrap_or_default()
    }
}

impl UnpackFromBytes for FileSearchResponse {
    fn unpack_from_bytes(buf: &mut Vec<u8>) -> Option<Self> {
        let mut bytes = Vec::new();
        ZlibDecoder::new(buf.as_slice())
            .read_to_end(&mut bytes)
            .ok()?;
        let username = <String>::unpack_from_bytes(&mut bytes)?;
        let token = <u32>::unpack_from_bytes(&mut bytes)?;
        let files = <Vec<File>>::unpack_from_bytes(&mut bytes)?;
        let slot_free = <bool>::unpack_from_bytes(&mut bytes)?;
        let avg_speed = <u32>::unpack_from_bytes(&mut bytes)?;
        let queue_length = <u32>::unpack_from_bytes(&mut bytes)?;
        let unknown_0 = <u32>::unpack_from_bytes(&mut bytes)?;
        let private_files = <Option<Vec<File>>>::unpack_from_bytes(&mut bytes)?;

        Some(Self {
            username,
            token,
            files,
            slot_free,
            avg_speed,
            queue_length,
            unknown_0,
            private_files,
        })
    }
}

impl_message_trait!(
    FileSearchResponse < FileSearchResponse,
    FileSearchResponse > (MessageType::Peer(9))
);

define_message_to_send_and_receive!(UserInfoRequest {});
impl_message_trait!(
    UserInfoRequest < UserInfoRequest,
    UserInfoRequest > (MessageType::Peer(15))
);

#[derive(Debug, Clone)]
pub struct Picture {
    pub picture: Option<String>,
}
impl PackToBytes for Picture {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match &self.picture {
            Some(picture) => {
                bytes.extend(true.pack_to_bytes());
                bytes.extend(picture.pack_to_bytes())
            }
            None => bytes.extend(false.pack_to_bytes()),
        }
        bytes
    }
}
impl UnpackFromBytes for Picture {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let exists = <bool>::unpack_from_bytes(bytes)?;
        let picture: Option<String> = match exists {
            true => Some(<String>::unpack_from_bytes(bytes)?),
            false => None,
        };
        Some(Picture { picture })
    }
}
define_message_to_send_and_receive!(UserInfoResponse {
    description: String,
    picture: Picture,
    upload_num: u32,
    queue_size: u32,
    slots_free: bool,
    upload_permitted: u32,
});
impl_message_trait!(
    UserInfoResponse < UserInfoResponse,
    UserInfoResponse > (MessageType::Peer(16))
);

define_message_to_send_and_receive!(FolderContentsRequest {
    token: u32,
    folder: String,
});
impl_message_trait!(
    FolderContentsRequest < FolderContentsRequest,
    FolderContentsRequest > (MessageType::Peer(36))
);

define_message_to_send_and_receive!(FolderContentsResponse {
    token: u32,
    folder: String,
    folders: Vec<Directory>,
});
impl_message_trait!(
    FolderContentsResponse < FolderContentsResponse,
    FolderContentsResponse > (MessageType::Peer(37))
);

#[derive(Debug, Clone)]
pub struct TransferRequest {
    pub direction: TransferDirections,
    pub token: u32,
    pub filename: String,
    pub filesize: Option<u64>,
}

impl PackToBytes for TransferRequest {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.direction.pack_to_bytes());
        bytes.extend(self.token.pack_to_bytes());
        bytes.extend(self.filename.pack_to_bytes());
        if self.direction == TransferDirections::UploadToPeer {
            bytes.extend(self.filesize.unwrap_or_default().pack_to_bytes())
        };
        bytes
    }
}

impl UnpackFromBytes for TransferRequest {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let direction = <TransferDirections>::unpack_from_bytes(bytes)?;
        let token = <u32>::unpack_from_bytes(bytes)?;
        let filename = <String>::unpack_from_bytes(bytes)?;
        let filesize: Option<u64>;
        if direction == TransferDirections::UploadToPeer {
            filesize = Some(<u64>::unpack_from_bytes(bytes)?);
        } else {
            filesize = None
        };
        Some(TransferRequest {
            direction,
            token,
            filename,
            filesize,
        })
    }
}
impl_message_trait!(
    TransferRequest < TransferRequest,
    TransferRequest > (MessageType::Peer(40))
);

#[derive(Debug, Clone)]
pub enum TransferResponseReason {
    Allowed(Option<u64>),
    NotAllowed(String),
}
impl PackToBytes for TransferResponseReason {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match &self {
            TransferResponseReason::Allowed(filesize) => {
                bytes.extend(true.pack_to_bytes());
                bytes.extend(filesize.pack_to_bytes());
            }
            TransferResponseReason::NotAllowed(reason) => {
                bytes.extend(false.pack_to_bytes());
                bytes.extend(reason.pack_to_bytes());
            }
        }
        bytes
    }
}
impl UnpackFromBytes for TransferResponseReason {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let allowed = <bool>::unpack_from_bytes(bytes)?;
        let reason = if allowed {
            TransferResponseReason::Allowed(<u64>::unpack_from_bytes(bytes))
        } else {
            TransferResponseReason::NotAllowed(<String>::unpack_from_bytes(bytes)?)
        };
        Some(reason)
    }
}

#[derive(Debug, Clone)]
pub struct TransferResponse {
    pub token: u32,
    pub reason: TransferResponseReason,
}
impl PackToBytes for TransferResponse {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.token.pack_to_bytes());
        match &self.reason {
            TransferResponseReason::Allowed(filesize) => {
                bytes.extend(true.pack_to_bytes());
                bytes.extend(filesize.pack_to_bytes());
            }
            TransferResponseReason::NotAllowed(reason) => {
                bytes.extend(false.pack_to_bytes());
                bytes.extend(reason.pack_to_bytes());
            }
        }
        bytes
    }
}
impl UnpackFromBytes for TransferResponse {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let token = <u32>::unpack_from_bytes(bytes)?;
        let reason = <TransferResponseReason>::unpack_from_bytes(bytes)?;
        Some(TransferResponse { token, reason })
    }
}
impl_message_trait!(
    TransferResponse < TransferResponse,
    TransferResponse > (MessageType::Peer(41))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(QueueUpload {
    filename: String,
});
impl_message_trait!(
    QueueUpload < QueueUpload,
    QueueUpload > (MessageType::Peer(43))
);

define_message_to_send_and_receive!(PlaceInQueueResponse {
    filename: String,
    place: u32,
});
impl_message_trait!(
    PlaceInQueueResponse < PlaceInQueueResponse,
    PlaceInQueueResponse > (MessageType::Peer(44))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(UploadFailed {
    filename: String,
});
impl_message_trait!(
    UploadFailed < UploadFailed,
    UploadFailed > (MessageType::Peer(46))
);

define_message_to_send_and_receive!(UploadDenied {
    filename: String,
    reason: String,
});
impl_message_trait!(
    UploadDenied < UploadDenied,
    UploadDenied > (MessageType::Peer(50))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(PlaceInQueueRequest {
    filename: String,
});
impl_message_trait!(
    PlaceInQueueRequest < PlaceInQueueRequest,
    PlaceInQueueRequest > (MessageType::Peer(51))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(UploadQueueNotification {});
impl_message_trait!(
    UploadQueueNotification < UploadQueueNotification,
    UploadQueueNotification > (MessageType::Peer(52))
);
