use std::collections::HashMap;

use crate::messages::UserStats;

#[derive(Default, Clone)]
pub(crate) struct ChatroomState {
    // pub(crate) title: &'a str,
    pub(crate) messages: Vec<String>,
    pub(crate) users: HashMap<String, UserStats>,
}

impl<'a> ChatroomState {
    pub(crate) fn add_message(&mut self, message: String) {
        self.messages.push(message)
    }

    pub(crate) fn add_user(&mut self, user: String, stats: UserStats) {
        self.users.insert(user, stats);
    }

    pub(crate) fn remove_user(&mut self, user: String) {
        self.users.remove(&user);
    }
}
