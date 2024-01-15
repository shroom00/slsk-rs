use crate::messages::UserStats;

#[derive(Clone, Debug)]
pub enum SLSKEvents {
    TryLogin { username: String, password: String },
    LoginResult { success: bool, reason: Option<String> },
    Quit,
    RoomList { rooms_and_num_of_users: Vec<(String, u32)> },
    /// Think of `private` like a boolean. 0 means public, anything else means private.
    JoinRoom { room: String, private: u32 },
    UpdateRoom { room: String, stats: Vec<(String, UserStats)> },
    /// Received when others send messages
    ChatroomMessage { room: String, username: Option<String>, message: String },
    // / Sent when you send messages
    // MessageChatroom {room: String, message: String },
}
