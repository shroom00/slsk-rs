use crate::packing::{PackToBytes, UnpackFromBytes};

use super::{MessageTrait, MessageType};

#[rustfmt::skip]
define_message_to_send_and_receive!(FileInit {
    token: u32,
});
// This is equivalent to both FileDownloadInit and FileUploadInit
// They're two sides of the same coin, although defined separately in the docs
impl_message_trait!(FileInit < FileInit, FileInit > (MessageType::File));

#[rustfmt::skip]
define_message_to_send_and_receive!(FileOffset {
    offset: u64,
});
impl_message_trait!(FileOffset < FileOffset, FileOffset > (MessageType::File));
