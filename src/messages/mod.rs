mod distributed;
mod file;
mod peer;
mod peer_init;
mod server;

use crate::{packing::PackToBytes, packing::UnpackFromBytes};
use async_trait::async_trait;
pub(crate) use distributed::*;
pub(crate) use file::*;
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
    fn write_to<W>(stream: &mut W, message: Self::ToSend) -> std::io::Result<()>
    where
        W: Write,
    {
        let mut data: Vec<u8> = vec![0, 0, 0, 0];
        data.extend(Self::CODE.pack_to_bytes());
        data.extend(message.pack_to_bytes());
        data.splice(..4, ((data.len() - 4) as u32).pack_to_bytes());
        stream.write_all(&data)
    }

    /// Does what write_to does but asynchronously
    async fn async_write_to<W>(
        stream: &'_ mut W,
        message: Self::ToSend,
    ) -> Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + '_>>
    where
        W: AsyncWriteExt + Unpin + Send,
    {
        let mut data: Vec<u8> = vec![0, 0, 0, 0];
        data.extend(Self::CODE.pack_to_bytes());
        data.extend(message.pack_to_bytes());
        data.splice(..4, ((data.len() - 4) as u32).pack_to_bytes());
        Box::pin(async move { stream.write_all(&data).await })
    }

    fn from_stream(stream: &mut Vec<u8>) -> Self::ToReceive
    where
        Self: Sized,
    {
        Self::ToReceive::unpack_from_bytes(stream)
    }
}

#[derive(Debug, Clone, Copy)]
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
            MessageType::Server(_) => MessageType::Server(<u32>::unpack_from_bytes(bytes)),
            MessageType::Peer(_) => MessageType::Peer(<u32>::unpack_from_bytes(bytes)),
            MessageType::PeerInit(_) | MessageType::Distributed(_) => {
                MessageType::PeerInit(<u8>::unpack_from_bytes(bytes))
            }
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
