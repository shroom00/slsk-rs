mod distributed;
mod file;
mod peer;
mod peer_init;
#[allow(dead_code)]
mod server;

use crate::{packing::PackToBytes, packing::UnpackFromBytes};
use async_trait::async_trait;
pub(crate) use peer::*;
pub(crate) use peer_init::*;
pub(crate) use server::*;
use std::{io::Write, pin::Pin};
use tokio::io::AsyncWriteExt;

#[async_trait]
pub trait MessageTrait: Sized {
    type ToSend: PackToBytes + Send;
    type ToReceive: UnpackFromBytes;
    const CODE: MessageType;

    fn to_bytes(message: Self::ToSend) -> Vec<u8> {
        let is_file = Self::CODE == MessageType::File;
        let mut data: Vec<u8> = if is_file { Vec::new() } else { vec![0, 0, 0, 0] };
        data.extend(Self::CODE.pack_to_bytes());
        data.extend(message.pack_to_bytes());
        if !is_file {
            data.splice(..4, ((data.len() - 4) as u32).pack_to_bytes());
        }
        data
    }

    #[allow(dead_code)]
    fn write_to<W>(stream: &mut W, message: Self::ToSend) -> std::io::Result<()>
    where
        W: Write,
    {
        let data = Self::to_bytes(message);
        stream.write_all(&data)
    }

    /// Does what write_to does but asynchronously
    async fn async_write_to<'a, W>(
        stream: &'a mut W,
        message: Self::ToSend,
    ) -> Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + 'a>>
    where
        W: AsyncWriteExt + Unpin + Send,
    {
        let data = Self::to_bytes(message);
        Box::pin(async move { stream.write_all(&data).await })
    }

    /// Returns `None` if there aren't enough bytes to unpack the object.
    fn from_stream(stream: &mut Vec<u8>) -> Option<Self::ToReceive>
    where
        Self: Sized,
    {
        Self::ToReceive::unpack_from_bytes(stream)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    Server(u32),
    PeerInit(u8),
    Peer(u32),
    File,
    Distributed(u8),
}

impl UnpackFromBytes for MessageType {
    fn unpack_from_bytes(_: &mut Vec<u8>) -> Option<Self>
    where
        Self: Sized,
    {
        unimplemented!("This only exists to make the trait complete and should not be used. Try _unpack_self_from_bytes instead.")
    }

    fn _unpack_self_from_bytes(&self, bytes: &mut Vec<u8>) -> Option<Self>
    where
        Self: Sized,
    {
        // We only care about the message type so we can get the right type.
        // The number is unimportant
        Some(match self {
            MessageType::Server(_) => MessageType::Server(<u32>::unpack_from_bytes(bytes)?),
            MessageType::Peer(_) => MessageType::Peer(<u32>::unpack_from_bytes(bytes)?),
            MessageType::PeerInit(_) | MessageType::Distributed(_) => {
                MessageType::PeerInit(<u8>::unpack_from_bytes(bytes)?)
            }
            MessageType::File => MessageType::File,
        })
    }
}

impl PackToBytes for MessageType {
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            MessageType::Server(u) | MessageType::Peer(u) => u.pack_to_bytes(),
            MessageType::PeerInit(u) | MessageType::Distributed(u) => vec![u.clone()],
            MessageType::File => Vec::new(),
        }
    }
}
