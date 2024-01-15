use crate::{
    constants::{ConnectionTypes, UserStatusCodes, MAJOR_VERSION, MINOR_VERSION},
    messages::{MessageTrait, MessageType},
    packing::IsntSent,
    packing::{IsntReceived, PackToBytes, UnpackFromBytes},
    utils::md5_digest,
};

use std::net::Ipv4Addr;

pub struct Login;

define_message_to_send!(_SendLogin {
    username: String,
    password: String,
    version: u32,
    hash: String,
    minor_version: u32,
});

impl _SendLogin {
    pub fn new(username: String, password: String) -> Self {
        let mut hash = username.clone();
        hash.push_str(&password);
        _SendLogin {
            username,
            password,
            version: MAJOR_VERSION,
            hash: md5_digest(hash.as_bytes()),
            minor_version: MINOR_VERSION,
        }
    }
}

generate_struct!(_ReceiveLogin {
    success: bool,
    greet: Option<String>,
    failure_reason: Option<String>,
    ip: Option<Ipv4Addr>,
    hash: Option<String>,
    is_supporter: Option<bool>,
});
// This needs to be implemented manually as the content varies
// depending on login success or failure
// This is too complex of a macro to make (if even possible)
// for a single use case
impl UnpackFromBytes for _ReceiveLogin {
    fn unpack_from_bytes(stream: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let success = <bool>::unpack_from_bytes(stream);

        if success {
            _ReceiveLogin {
                success,
                greet: Some(<String>::unpack_from_bytes(stream)),
                failure_reason: None,
                ip: Some(<Ipv4Addr>::unpack_from_bytes(stream)),
                hash: Some(<String>::unpack_from_bytes(stream)),
                is_supporter: Some(<bool>::unpack_from_bytes(stream)),
            }
        } else {
            _ReceiveLogin {
                success,
                greet: None,
                failure_reason: Some(<String>::unpack_from_bytes(stream)),
                ip: None,
                hash: None,
                is_supporter: None,
            }
        }
    }
}

impl_message_trait!(Login < _SendLogin, _ReceiveLogin > (MessageType::Server(1)));

define_message_to_send!(SetWaitPort {
    port: u32,
    use_obfuscation: bool,
    obfuscated_port: u32,
});
impl_message_trait!(
    SetWaitPort < SetWaitPort,
    IsntReceived > (MessageType::Server(2))
);

#[rustfmt::skip]
define_message_to_send!(_SendGetPeerAddress { username: String ,});

define_message_to_receive!(_ReceiveGetPeerAddress {
    username: String,
    ip: Ipv4Addr,
    port: u32,
    use_obfuscation: bool,
    obfuscated_port: u32,
});

pub struct GetPeerAddress;
impl_message_trait!(
    GetPeerAddress < _SendGetPeerAddress,
    _ReceiveGetPeerAddress > (MessageType::Server(3))
);

#[rustfmt::skip]
define_message_to_send!(_SendWatchUser { username: String ,});

generate_struct!(_ReceiveWatchUser {
    username: String,
    exists: bool,
    status: Option<UserStatusCodes>,
    avg_speed: Option<u32>,
    upload_num: Option<u64>,
    files: Option<u32>,
    dirs: Option<u32>,
    country_code: Option<String>,
});
impl UnpackFromBytes for _ReceiveWatchUser {
    fn unpack_from_bytes(stream: &mut Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let username = <String>::unpack_from_bytes(stream);
        let exists = <bool>::unpack_from_bytes(stream);
        let status: Option<UserStatusCodes>;
        let avg_speed: Option<u32>;
        let upload_num: Option<u64>;
        let files: Option<u32>;
        let dirs: Option<u32>;
        let country_code: Option<String>;
        if exists {
            status = Some(<UserStatusCodes>::unpack_from_bytes(stream));
            avg_speed = Some(<u32>::unpack_from_bytes(stream));
            upload_num = Some(<u64>::unpack_from_bytes(stream));
            files = Some(<u32>::unpack_from_bytes(stream));
            dirs = Some(<u32>::unpack_from_bytes(stream));
            if status != Some(UserStatusCodes::Offline) {
                country_code = Some(<String>::unpack_from_bytes(stream));
            } else {
                country_code = None;
            }
        } else {
            status = None;
            avg_speed = None;
            upload_num = None;
            files = None;
            dirs = None;
            country_code = None;
        };
        _ReceiveWatchUser {
            username,
            exists,
            status,
            avg_speed,
            upload_num,
            files,
            dirs,
            country_code,
        }
    }
}

pub struct WatchUser;
impl_message_trait!(
    WatchUser < _SendWatchUser,
    _ReceiveWatchUser > (MessageType::Server(5))
);

#[rustfmt::skip]
define_message_to_send!(UnwatchUser { username: String ,});
impl_message_trait!(
    UnwatchUser < UnwatchUser,
    IsntReceived > (MessageType::Server(6))
);

#[rustfmt::skip]
define_message_to_send!(_SendGetUserStatus { username: String ,});

define_message_to_receive!(_ReceiveGetUserStatus {
    username: String,
    status: UserStatusCodes,
    privileged: bool,
});

pub struct GetUserStatus;
impl_message_trait!(
    GetUserStatus < _SendGetUserStatus,
    _ReceiveGetUserStatus > (MessageType::Server(7))
);

define_message_to_send!(_SendSayChatroom {
    room: String,
    message: String,
});

define_message_to_receive!(_ReceiveSayChatroom {
    room: String,
    username: String,
    message: String,
});

pub struct SayChatroom;
impl_message_trait!(
    SayChatroom < _SendSayChatroom,
    _ReceiveSayChatroom > (MessageType::Server(13))
);

define_message_to_send!(_SendJoinRoom {
    room: String,
    private: u32,
});

define_message_to_receive!(UserStats {
    avg_speeds: u32,
    upload_num: u64, // Is this the same as num_of_files?
    num_of_files: u32,
    num_of_dirs: u32,
});
#[rustfmt::skip]
pub struct _ReceiveJoinRoom {
    pub room: String,
    pub usernames: Vec<String>,
    pub statuses: Vec<UserStatusCodes>,
    pub stats: Vec<UserStats>,
    pub slots_free: Vec<u32>,
    pub country_codes: Vec<String>,
    pub owner:  Option<String>, // Only exists if room is private.
    pub operators: Option<Vec<String>>,  // Only exists if room is private.
}

impl UnpackFromBytes for _ReceiveJoinRoom {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let room = <String>::unpack_from_bytes(bytes);
        let usernames = <Vec<String>>::unpack_from_bytes(bytes);
        let statuses = <Vec<UserStatusCodes>>::unpack_from_bytes(bytes);
        let stats = <Vec<UserStats>>::unpack_from_bytes(bytes);
        let slots_free = <Vec<u32>>::unpack_from_bytes(bytes);
        let country_codes = <Vec<String>>::unpack_from_bytes(bytes);
        let owner: Option<String>;
        let operators: Option<Vec<String>>;
        if bytes.len() == 0 {
            owner = None;
            operators = None;
        } else {
            owner = Some(<String>::unpack_from_bytes(bytes));
            operators = Some(<Vec<String>>::unpack_from_bytes(bytes));
        };
        Self {
            room,
            usernames,
            statuses,
            stats,
            slots_free,
            country_codes,
            owner,
            operators,
        }
    }
}
pub struct JoinRoom;
impl_message_trait!(
    JoinRoom < _SendJoinRoom,
    _ReceiveJoinRoom > (MessageType::Server(14))
);

#[rustfmt::skip]
define_message_to_send!(_SendLeaveRoom { room: String ,});

#[rustfmt::skip]
define_message_to_receive!(_ReceiveLeaveRoom { room: String ,});

pub struct LeaveRoom;
impl_message_trait!(
    LeaveRoom < _SendLeaveRoom,
    _ReceiveLeaveRoom > (MessageType::Server(15))
);

define_message_to_receive!(UserJoinedRoom {
    room: String,
    username: String,
    status: UserStatusCodes,
    avg_speed: u32,
    upload_num: u64,
    files: u32,
    dirs: u32,
    slots_free: u32,
    country_code: String,
});
impl_message_trait!(
    UserJoinedRoom < IsntSent,
    UserJoinedRoom > (MessageType::Server(16))
);

define_message_to_receive!(UserLefRoom {
    room: String,
    username: String,
});
impl_message_trait!(
    UserLefRoom < IsntSent,
    UserLefRoom > (MessageType::Server(17))
);

define_message_to_send!(_SendConnectToPeer {
    token: u32,
    username: String,
    connection_type: ConnectionTypes,
});

define_message_to_receive!(_ReceiveConnectToPeer {
    username: String,
    connection_type: ConnectionTypes,
    ip: Ipv4Addr,
    port: u32,
    firewall_token: u32,
    privileged: bool,
    use_obfuscation: bool,
    obfuscated_port: u32,
});

pub struct ConnectToPeer;
impl_message_trait!(
    ConnectToPeer < _SendConnectToPeer,
    _ReceiveConnectToPeer > (MessageType::Server(18))
);

define_message_to_send!(_SendMessageUser {
    username: String,
    message: String,
});

define_message_to_receive!(_ReceiveMessageUser {
    id: u32,
    timestamp: u32,
    username: String,
    message: String,
    is_new: bool,
});

pub struct MessageUser;
impl_message_trait!(
    MessageUser < _SendMessageUser,
    _ReceiveMessageUser > (MessageType::Server(22))
);

#[rustfmt::skip]
define_message_to_send!(MessageAcked { message_id: u32 ,});
impl_message_trait!(
    MessageAcked < MessageAcked,
    IsntReceived > (MessageType::Server(23))
);

define_message_to_send!(_SendFileSearch {
    token: u32,
    search_query: String,
});

define_message_to_receive!(_ReceiveFileSearch {
    username: String,
    token: u32,
    search_query: String,
});

pub struct FileSearch;
impl_message_trait!(
    FileSearch < _SendFileSearch,
    _ReceiveFileSearch > (MessageType::Server(26))
);

define_message_to_send!(SetStatus {
    status: UserStatusCodes,
});
impl_message_trait!(
    SetStatus < SetStatus,
    IsntReceived > (MessageType::Server(28))
);

define_message_to_send!(ServerPing {});
impl_message_trait!(
    ServerPing < ServerPing,
    IsntReceived > (MessageType::Server(32))
);

define_message_to_send!(SharedFoldersFiles {
    dirs: u32,
    files: u32,
});
impl_message_trait!(
    SharedFoldersFiles < SharedFoldersFiles,
    IsntReceived > (MessageType::Server(35))
);

#[rustfmt::skip]
define_message_to_send!(_SendGetUserStats { username: String ,});

define_message_to_receive!(_ReceiveGetUserStats {
    username: String,
    avg_speed: u32,
    upload_num: u64,
    files: u32,
    dirs: u32,
});

pub struct GetUserStats;
impl_message_trait!(
    GetUserStats < _SendGetUserStats,
    _ReceiveGetUserStats > (MessageType::Server(36))
);

define_message_to_receive!(Relogged {});
impl_message_trait!(Relogged < IsntSent, Relogged > (MessageType::Server(41)));

define_message_to_send!(UserSearch {
    username: String,
    token: u32,
    search_query: String,
});
impl_message_trait!(
    UserSearch < UserSearch,
    IsntReceived > (MessageType::Server(42))
);

// TODO:
// Server Codes 51, 52, 56, 57 all deprecated and sent only.
// Can be ignored for now, "maybe" added in the future.
// The following are deprecated but can still be received,
// should be implemented and handled.
// 54: Recommendations
// 56: GlobalRecommendations
// 57: UserInterests
define_message_to_send!(_SendRoomList {});
// This message is a mess, I hate it!
#[rustfmt::skip]
define_message_to_receive!(_ReceiveRoomList {
    rooms: Vec<String>,
    num_of_users: Vec<u32>,

    owned_priv_rooms: Vec<String>,
    owned_priv_num_of_users: Vec<u32>,

    non_owned_priv_rooms: Vec<String>,
    non_owned_priv_num_of_users: Vec<u32>,

    operated_priv_rooms: Vec<String>,
});
pub struct RoomList;
impl_message_trait!(
    RoomList < _SendRoomList,
    _ReceiveRoomList > (MessageType::Server(64))
);
#[rustfmt::skip]

define_message_to_receive!(AdminMessage {
    message: String,
});
impl_message_trait!(
    AdminMessage < IsntSent,
    AdminMessage > (MessageType::Server(66))
);

#[rustfmt::skip]
define_message_to_receive!(PrivilegedUsers {
    usernames: Vec<String>,
});
impl_message_trait!(
    PrivilegedUsers < IsntSent,
    PrivilegedUsers > (MessageType::Server(69))
);

#[rustfmt::skip]
define_message_to_send!(HaveNoParent {
    have_parents: bool,
});
impl_message_trait!(
    HaveNoParent < HaveNoParent,
    IsntReceived > (MessageType::Server(71))
);

// Server Code 73 deprecated

#[rustfmt::skip]
define_message_to_receive!(ParentMinSpeed {
    speed: u32, // Mbps?
});
impl_message_trait!(
    ParentMinSpeed < IsntSent,
    ParentMinSpeed > (MessageType::Server(83))
);

#[rustfmt::skip]
define_message_to_receive!(ParentSpeedRatio {
    ratio: u32,
});
impl_message_trait!(
    ParentSpeedRatio < IsntSent,
    ParentSpeedRatio > (MessageType::Server(84))
);

define_message_to_send!(_SendCheckPrivileges {});
define_message_to_receive!(_ReceiveCheckPrivileges {
    time_left_seconds: u32,
});
pub struct CheckPrivileges;
impl_message_trait!(
    CheckPrivileges < _SendCheckPrivileges,
    _ReceiveCheckPrivileges > (MessageType::Server(92))
);

define_message_to_receive!(EmbeddedMessage {
    distributed_code: u8,
    distributed_message: String, // This is actually raw bytes, is String essentially the same?
});
impl_message_trait!(
    EmbeddedMessage < IsntSent,
    EmbeddedMessage > (MessageType::Server(93))
);

#[rustfmt::skip]
define_message_to_send!(AcceptChildren {
    accept: bool,
});
impl_message_trait!(
    AcceptChildren < AcceptChildren,
    IsntReceived > (MessageType::Server(100))
);

#[rustfmt::skip]
define_message_to_receive!(PossibleParents {
    usernames: Vec<String>,
});
impl_message_trait!(
    PossibleParents < IsntSent,
    PossibleParents > (MessageType::Server(102))
);

define_message_to_send!(WishlistSearch {
    token: u32,
    search_query: u32,
});
impl_message_trait!(
    WishlistSearch < WishlistSearch,
    IsntReceived > (MessageType::Server(103))
);

define_message_to_receive!(WishListInterval {
    interval: u32, // Seconds?
});
impl_message_trait!(
    WishListInterval < IsntSent,
    WishListInterval > (MessageType::Server(104))
);

// TODO:
// The following are deprecated but can be received.
// 110: SimilarUsers
// 111: ItemRecommendations
// 112: ItemSimilarUsers

define_message_to_receive!(Ticker {
    username: String,
    ticker: String,
});
define_message_to_receive!(RoomTickerState {
    room: String,
    tickers: Vec<Ticker>,
});
impl_message_trait!(
    RoomTickerState < IsntSent,
    RoomTickerState > (MessageType::Server(113))
);

define_message_to_receive!(RoomTickerAdd {
    room: String,
    username: String,
    ticker: String,
});
impl_message_trait!(
    RoomTickerAdd < IsntSent,
    RoomTickerAdd > (MessageType::Server(114))
);

define_message_to_receive!(RoomTickerRemove {
    room: String,
    username: String,
});
impl_message_trait!(
    RoomTickerRemove < IsntSent,
    RoomTickerRemove > (MessageType::Server(115))
);

define_message_to_send!(RoomTickerSet {
    room: String,
    ticker: String,
});
impl_message_trait!(
    RoomTickerSet < RoomTickerSet,
    IsntReceived > (MessageType::Server(116))
);

// Server Codes 117, 118 are deprecated

define_message_to_send!(RoomSearch {
    room: String,
    token: u32,
    search_query: String,
});
impl_message_trait!(
    RoomSearch < RoomSearch,
    IsntReceived > (MessageType::Server(120))
);

#[rustfmt::skip]
define_message_to_send!(SendUploadSpeed {
    speed: u32,
});
impl_message_trait!(
    SendUploadSpeed < SendUploadSpeed,
    IsntReceived > (MessageType::Server(121))
);

// TODO:
// Server Code 122 (UserPrivileged) is deprecated but can still be received

define_message_to_send!(GivePrivileges {
    username: String,
    days: u32,
});
impl_message_trait!(
    GivePrivileges < GivePrivileges,
    IsntReceived > (MessageType::Server(123))
);

// TODO:
// Server Code 124 (NotifyPrivileges), is deprecated but can still be received
// 125 (AckNotifyPrivileges) is "no longer used" but can technically still be received.
// Not sure if it needs implementing
#[rustfmt::skip]
define_message_to_send!(BranchLevel {
    branch_level: u32,
});
impl_message_trait!(
    BranchLevel < BranchLevel,
    IsntReceived > (MessageType::Server(126))
);

define_message_to_send!(BranchRoot {
    branch_root: String,
});
impl_message_trait!(
    BranchRoot < BranchRoot,
    IsntReceived > (MessageType::Server(127))
);

// Server Code 129 is deprecated and not received

define_message_to_receive!(ResetDistributed {});
impl_message_trait!(
    ResetDistributed < IsntSent,
    ResetDistributed > (MessageType::Server(130))
);

#[rustfmt::skip]
define_message_to_receive!(PrivateRoomUsers {
    room: String,
    users: Vec<String>,
});
impl_message_trait!(
    PrivateRoomUsers < IsntSent,
    PrivateRoomUsers > (MessageType::Server(133))
);

define_message_to_send_and_receive!(PrivateRoomAddUser {
    room: String,
    username: String,
});
impl_message_trait!(
    PrivateRoomAddUser < PrivateRoomAddUser,
    PrivateRoomAddUser > (MessageType::Server(134))
);

define_message_to_send_and_receive!(PrivateRoomRemoveUser {
    room: String,
    username: String,
});
impl_message_trait!(
    PrivateRoomRemoveUser < PrivateRoomRemoveUser,
    PrivateRoomRemoveUser > (MessageType::Server(135))
);

#[rustfmt::skip]
define_message_to_send!(PrivateRoomDismember {
    room: String,
});
impl_message_trait!(
    PrivateRoomDismember < PrivateRoomDismember,
    IsntReceived > (MessageType::Server(136))
);

#[rustfmt::skip]
define_message_to_send!(PrivateRoomDisown {
    room: String,
});
impl_message_trait!(
    PrivateRoomDisown < PrivateRoomDisown,
    IsntReceived > (MessageType::Server(137))
);

#[rustfmt::skip]
define_message_to_receive!(PrivateRoomAdded {
    room: String,
});
impl_message_trait!(
    PrivateRoomAdded < IsntSent,
    PrivateRoomAdded > (MessageType::Server(139))
);

#[rustfmt::skip]
define_message_to_receive!(PrivateRoomRemoved {
    room: String,
});
impl_message_trait!(
    PrivateRoomRemoved < IsntSent,
    PrivateRoomRemoved > (MessageType::Server(140))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(PrivateRoomToggle {
    enable: bool,
});
impl_message_trait!(
    PrivateRoomToggle < PrivateRoomToggle,
    PrivateRoomToggle > (MessageType::Server(141))
);

#[rustfmt::skip]
define_message_to_send_and_receive!(ChangePassword {
    password: String,
});
impl_message_trait!(
    ChangePassword < ChangePassword,
    ChangePassword > (MessageType::Server(142))
);

define_message_to_send_and_receive!(PrivateRoomAddOperator {
    room: String,
    username: String,
});
impl_message_trait!(
    PrivateRoomAddOperator < PrivateRoomAddOperator,
    PrivateRoomAddOperator > (MessageType::Server(143))
);

define_message_to_send_and_receive!(PrivateRoomRemoveOperator {
    room: String,
    username: String,
});
impl_message_trait!(
    PrivateRoomRemoveOperator < PrivateRoomRemoveOperator,
    PrivateRoomRemoveOperator > (MessageType::Server(144))
);

#[rustfmt::skip]
define_message_to_receive!(PrivateRoomOperatorAdded {
    room: String,
});
impl_message_trait!(
    PrivateRoomOperatorAdded < IsntSent,
    PrivateRoomOperatorAdded > (MessageType::Server(145))
);

#[rustfmt::skip]
define_message_to_receive!(PrivateRoomOperatorRemoved {
    room: String,
});
impl_message_trait!(
    PrivateRoomOperatorRemoved < IsntSent,
    PrivateRoomOperatorRemoved > (MessageType::Server(146))
);

#[rustfmt::skip]
define_message_to_receive!(PrivateRoomOwned {
    room: String,
    operators: Vec<String>,
});
impl_message_trait!(
    PrivateRoomOwned < IsntSent,
    PrivateRoomOwned > (MessageType::Server(148))
);

#[rustfmt::skip]
define_message_to_send!(MessageUsers {
    usernames: Vec<String>,
    message: String,
});
impl_message_trait!(
    MessageUsers < MessageUsers,
    IsntReceived > (MessageType::Server(149))
);

// TODO:
// Server Codes 150, 151 are deprecated and not received
// 152 (GlobalRoomMessage) deprecated but received

define_message_to_send_and_receive!(CantConnectToPeer {
    token: u32,
    username: String,
});
impl_message_trait!(
    CantConnectToPeer < CantConnectToPeer,
    CantConnectToPeer > (MessageType::Server(1001))
);

#[rustfmt::skip]
define_message_to_receive!(CantConnectToRoom {
    room: String,
});
impl_message_trait!(
    CantConnectToRoom < IsntSent,
    CantConnectToRoom > (MessageType::Server(1003))
);
