use super::{macros::PackToBytes, MessageTrait, MessageType};
use crate::{
    constants::{FileAttributeTypes, TransferDirections},
    packing::UnpackFromBytes,
};

define_message_to_send_and_receive!(GetSharedFileList {});
impl_message_trait!(
    GetSharedFileList < GetSharedFileList,
    GetSharedFileList > (MessageType::Peer(4))
);

define_message_to_send_and_receive!(FileAttribute {
    attribute: FileAttributeTypes,
    value: u32,
});
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
define_message_to_send_and_receive!(SharedFileListResponse {
    directories: Vec<Directory>,
    unknown_0: u32,
    priv_directories: Vec<Directory>,
});
impl_message_trait!(
    SharedFileListResponse < SharedFileListResponse,
    SharedFileListResponse > (MessageType::Peer(5))
);

define_message_to_send_and_receive!(FileSearchResponse {
    username: String,
    token: u32,
    files: Vec<File>,
    slot_free: bool,
    avg_speed: u32,
    queue_length: u32,
    unknown_0: u32,
    private_files: Vec<File>,
});
impl_message_trait!(
    FileSearchResponse < FileSearchResponse,
    FileSearchResponse > (MessageType::Peer(9))
);

define_message_to_send_and_receive!(UserInfoRequest {});
impl_message_trait!(
    UserInfoRequest < UserInfoRequest,
    UserInfoRequest > (MessageType::Peer(15))
);

#[derive(Debug)]
pub struct Picture {
    pub picture: Option<String>,
}
impl PackToBytes for Picture {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
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
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let exists = <bool>::unpack_from_bytes(bytes);
        let picture: Option<String> = match exists {
            true => Some(<String>::unpack_from_bytes(bytes)),
            false => None,
        };
        Picture { picture }
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

#[derive(Debug)]
pub struct TransferRequest {
    pub direction: TransferDirections,
    pub token: u32,
    pub filename: String,
    pub filesize: Option<u64>,
}

impl PackToBytes for TransferRequest {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
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
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let direction = <TransferDirections>::unpack_from_bytes(bytes);
        let token = <u32>::unpack_from_bytes(bytes);
        let filename = <String>::unpack_from_bytes(bytes);
        let filesize: Option<u64>;
        if direction == TransferDirections::UploadToPeer {
            filesize = Some(<u64>::unpack_from_bytes(bytes));
        } else {
            filesize = None
        };
        TransferRequest {
            direction,
            token,
            filename,
            filesize,
        }
    }
}
impl_message_trait!(
    TransferRequest < TransferRequest,
    TransferRequest > (MessageType::Peer(40))
);

#[derive(Debug)]
pub struct TransferResponse {
    token: u32,
    allowed: bool,
    reason: Option<String>,
}
impl PackToBytes for TransferResponse {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend(self.token.pack_to_bytes());
        bytes.extend(self.allowed.pack_to_bytes());
        if !self.allowed {
            bytes.extend(self.reason.clone().unwrap_or_default().pack_to_bytes());
        };
        bytes
    }
}
impl UnpackFromBytes for TransferResponse {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let token = <u32>::unpack_from_bytes(bytes);
        let allowed = <bool>::unpack_from_bytes(bytes);
        let reason: Option<String>;
        if allowed {
            reason = None;
        } else {
            reason = Some(<String>::unpack_from_bytes(bytes));
        };
        TransferResponse {
            token,
            allowed,
            reason,
        }
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
define_message_to_send_and_receive!(PlaceInQueuRequest {
    filename: String,
});
impl_message_trait!(
    PlaceInQueuRequest < PlaceInQueuRequest,
    PlaceInQueuRequest > (MessageType::Peer(51))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(UploadQueueNotification {});
impl_message_trait!(
    UploadQueueNotification < UploadQueueNotification,
    UploadQueueNotification > (MessageType::Peer(52))
);
