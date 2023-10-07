use crate::packing::{PackToBytes, UnpackFromBytes};

use super::{MessageTrait, MessageType};

define_message_to_send!(_SendDistribPing {});
define_message_to_receive!(_ReceiveDistribPing {});
pub struct DistribPing;
impl_message_trait!(
    DistribPing < _SendDistribPing,
    _ReceiveDistribPing > (MessageType::Distributed(0))
);

define_message_to_send_and_receive!(DistribSearch {
    unknown: u32,
    username: String,
    token: u32,
    query: String,
});
impl_message_trait!(
    DistribSearch < DistribSearch,
    DistribSearch > (MessageType::Distributed(3))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(DistribBranchLevel {
    branch_level: u32,
});
impl_message_trait!(
    DistribBranchLevel < DistribBranchLevel,
    DistribBranchLevel > (MessageType::Distributed(5))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(DistribChildDepth {
    child_depth: u32,
});
impl_message_trait!(
    DistribChildDepth < DistribChildDepth,
    DistribChildDepth > (MessageType::Distributed(7))
);

define_message_to_send_and_receive!(DistribEmbeddedMessage {
    distributed_code: MessageType,
    distributed_message: String, // Are raw bytes (un)packed the same as strings? No specific method is mentioned
});
impl_message_trait!(
    DistribEmbeddedMessage < DistribEmbeddedMessage,
    DistribEmbeddedMessage > (MessageType::Distributed(93))
);
