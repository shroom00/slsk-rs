pub(crate) mod button;
pub(crate) mod chatroom;
pub(crate) mod dropdown;
pub(crate) mod input;
pub(crate) mod table;
pub(crate) mod tabs;
pub(crate) mod list;

pub(crate) trait SelectItem {
    fn get_index(&self) -> Option<usize>;

    fn set_index(&mut self, index: Option<usize>);

    fn select_previous(&mut self);

    fn select_next(&mut self);
}
