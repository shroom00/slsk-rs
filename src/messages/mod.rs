/// Contains all the messages sent/received by the client

#[macro_use]
mod macros;
mod server;
mod peer_init;
mod peer;
mod file;
mod distributed;

use crate::{packing::UnpackFromBytes, packing::PackToBytes};
pub(crate) use server::*;
pub(crate) use peer_init::*;
pub(crate) use peer::*;
pub(crate) use file::*;
pub(crate) use distributed::*;
use std::{io::Write, net::TcpStream};

pub trait MessageTrait: Sized {
    type ToSend: PackToBytes;
    type ToReceive: UnpackFromBytes;
    const CODE: MessageType;
    fn to_stream(mut stream: &TcpStream, message: Self::ToSend) -> std::io::Result<()> {
        let mut data: Vec<u8> = vec![0, 0, 0, 0];
        data.extend(Self::CODE.pack_to_bytes());
        data.extend(message.pack_to_bytes());
        data.splice(..4, ((data.len() - 4) as u32).pack_to_bytes());
        stream.write_all(&data)
    }

    fn from_stream(stream: &mut Vec<u8>) -> Self::ToReceive
    where
        Self: Sized,
    {
        Self::ToReceive::unpack_from_bytes(stream)
    }
}

#[derive(Debug)]
pub enum MessageType {
    Server(u32),
    PeerInit(u8),
    Peer(u32),
    File,
    Distributed(u8),
}

impl UnpackFromBytes for MessageType {
    fn unpack_from_bytes(_: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        todo!("This only exists to make the trait complete and should not be used. Try _unpack_self_from_bytes instead.")
    }

    fn _unpack_self_from_bytes(&self, bytes: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        // We only care about the message type so we can gett the right type.
        // The number is unimportant
        match self {
            MessageType::Server(_) | MessageType::Peer(_) => MessageType::Server(<u32>::unpack_from_bytes(bytes)),
            MessageType::PeerInit(_) | MessageType::Distributed(_) => MessageType::PeerInit(<u8>::unpack_from_bytes(bytes)),
            MessageType::File => MessageType::File,
        }
    }
}

impl PackToBytes for MessageType {
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            MessageType::Server(u) | MessageType::Peer(u) => u.pack_to_bytes(),
            MessageType::PeerInit(u) | MessageType::Distributed(u) => vec![u.clone()],
            MessageType::File => vec![],
        }
    }
}
