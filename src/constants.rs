use crate::{packing::PackToBytes, packing::UnpackFromBytes};

pub(crate) const MAJOR_VERSION: u32 = 160;
pub(crate) const MINOR_VERSION: u32 = 1;

#[derive(Debug, PartialEq)]
pub enum UserStatusCodes {
    Offline,
    Away,
    Online,
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

pub enum TransferDirections {
    DownloadFromPeer,
    UploadToPeer,
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
            // should cause the connection to fail/timeout
            // For now, this seems safe
            _ => Self::PeerToPeer,
        }
    }
}
