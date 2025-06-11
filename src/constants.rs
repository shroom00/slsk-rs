use std::ops::Add;

use crate::{packing::{PackToBytes, UnpackFromBytes}, utils::num_as_bytes};

pub(crate) const MAJOR_VERSION: u32 = 160;
pub(crate) const MINOR_VERSION: u32 = 1;
pub(crate) const MAX_RESULTS: u32 = 1500;

#[derive(Debug, PartialEq, Clone)]
pub enum UserStatusCodes {
    Offline,
    Away,
    Online,
}

impl Default for UserStatusCodes {
    fn default() -> Self {
        Self::Offline
    }
}

impl UnpackFromBytes for UserStatusCodes {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self>
    where
        Self: Sized,
    {
        let u = <u32>::unpack_from_bytes(bytes)?;
        Some(match u {
            0 => Self::Offline,
            1 => Self::Away,
            2 => Self::Online,
            _ => Self::Offline,
        })
    }
}

impl PackToBytes for UserStatusCodes {
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            UserStatusCodes::Offline => 0.pack_to_bytes(),
            UserStatusCodes::Away => 1.pack_to_bytes(),
            UserStatusCodes::Online => 2.pack_to_bytes(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TransferDirections {
    DownloadFromPeer,
    UploadToPeer,
}
impl PackToBytes for TransferDirections {
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            TransferDirections::DownloadFromPeer => 0u32.pack_to_bytes(),
            TransferDirections::UploadToPeer => 1u32.pack_to_bytes(),
        }
    }
}

impl UnpackFromBytes for TransferDirections {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let u = <u32>::unpack_from_bytes(bytes)?;
        match u {
            0 => Some(Self::DownloadFromPeer),
            1 => Some(Self::UploadToPeer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionTypes {
    PeerToPeer,
    FileTransfer,
    DistributedNetwork,
}

impl PackToBytes for ConnectionTypes {
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            ConnectionTypes::PeerToPeer => String::from("P").pack_to_bytes(),
            ConnectionTypes::FileTransfer => String::from("F").pack_to_bytes(),
            ConnectionTypes::DistributedNetwork => String::from("D").pack_to_bytes(),
        }
    }
}

impl UnpackFromBytes for ConnectionTypes {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self>
    where
        Self: Sized,
    {
        let string = <String>::unpack_from_bytes(bytes)?;
        Some(match string.as_str() {
            "P" => Self::PeerToPeer,
            "F" => Self::FileTransfer,
            "D" => Self::DistributedNetwork,
            // TODO:
            // Defaults to P2P
            // When forming connections is actually implemented,
            // this should cause the connection to fail/timeout
            // For now, this seems safe but will need to check
            // when the backend is properly made
            _ => Self::PeerToPeer,
        })
    }
}

#[derive(Debug, Clone)]
pub enum FileAttributeTypes {
    /// Kbps
    Bitrate,
    /// Seconds
    Duration,
    /// 0 or 1
    VBR,
    Encoder,
    /// Hz
    SampleRate,
    /// Bits
    BitDepth,
}

impl PackToBytes for FileAttributeTypes {
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            FileAttributeTypes::Bitrate => 0.pack_to_bytes(),
            FileAttributeTypes::Duration => 1.pack_to_bytes(),
            FileAttributeTypes::VBR => 2.pack_to_bytes(),
            FileAttributeTypes::Encoder => 3.pack_to_bytes(),
            FileAttributeTypes::SampleRate => 4.pack_to_bytes(),
            FileAttributeTypes::BitDepth => 5.pack_to_bytes(),
        }
    }
}

impl UnpackFromBytes for FileAttributeTypes {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self>
    where
        Self: Sized,
    {
        let u = <u32>::unpack_from_bytes(bytes)?;
        match u {
            0 => Some(Self::Bitrate),
            1 => Some(Self::Duration),
            2 => Some(Self::VBR),
            3 => Some(Self::Encoder),
            4 => Some(Self::SampleRate),
            5 => Some(Self::BitDepth),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DownloadStatus {
    Queued,
    Starting,
    Downloading,
    Complete,
    Failed,
}

impl DownloadStatus {
    pub(crate) fn str(&self) -> &'static str {
        match *self {
            DownloadStatus::Queued => "Queued",
            DownloadStatus::Starting => "Starting",
            DownloadStatus::Downloading => "Downloading",
            DownloadStatus::Complete => "Complete",
            DownloadStatus::Failed => "Failed",
        }
    }
}

impl ToString for DownloadStatus {
    fn to_string(&self) -> String {
        self.str().to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ByteSize(pub(crate) u64);

impl ToString for ByteSize {
    fn to_string(&self) -> String {
        num_as_bytes(self.0)
    }
}

impl Add for ByteSize {
    type Output = ByteSize;

    fn add(self, rhs: Self) -> Self::Output {
        ByteSize(self.0 + rhs.0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Percentage(pub(crate) u8);

impl ToString for Percentage {
    fn to_string(&self) -> String {
        format!("{}%", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Token(pub(crate) u32);

impl ToString for Token {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}
