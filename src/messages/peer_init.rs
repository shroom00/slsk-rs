use super::{macros::PackToBytes, MessageTrait, MessageType};
use crate::{constants::ConnectionTypes, packing::UnpackFromBytes};

#[rustfmt::skip]
define_message_to_send_and_receive!(PierceFireWall {
    token: u32,
});
impl_message_trait!(
    PierceFireWall < PierceFireWall,
    PierceFireWall > (MessageType::PeerInit(0))
);

define_message_to_send_and_receive!(PeerInit {
    username: String,
    conn_type: ConnectionTypes,
    token: u32,
});
impl_message_trait!(PeerInit < PeerInit, PeerInit > (MessageType::PeerInit(1)));
