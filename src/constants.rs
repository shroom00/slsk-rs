use crate::{packing::PackToBytes, packing::UnpackFromBytes};

pub(crate) const MAJOR_VERSION: u32 = 160;
pub(crate) const MINOR_VERSION: u32 = 1;

#[derive(Debug, PartialEq)]
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
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let u = <u32>::unpack_from_bytes(bytes);
        match u {
            0 => Self::Offline,
            1 => Self::Away,
            2 => Self::Online,
            _ => Self::Offline,
        }
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

#[derive(Debug, PartialEq)]
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
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let u = <u32>::unpack_from_bytes(bytes);
        match u {
            0 => Self::DownloadFromPeer,
            1 => Self::UploadToPeer,
            _ => todo!(),
        }
    }
}

#[derive(Debug)]
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
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let string = <String>::unpack_from_bytes(bytes);
        match string.as_str() {
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
        }
    }
}

#[derive(Debug)]
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
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let u = <u32>::unpack_from_bytes(bytes);
        match u {
            0 => Self::Bitrate,
            1 => Self::Duration,
            2 => Self::VBR,
            3 => Self::Encoder,
            4 => Self::SampleRate,
            5 => Self::BitDepth,
            _ => todo!(),
        }
    }
}
