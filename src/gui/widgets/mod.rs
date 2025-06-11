pub(crate) mod button;
pub(crate) mod chatrooms;
pub(crate) mod dropdown;
pub(crate) mod input;
pub(crate) mod table;
pub(crate) mod tabs;
pub(crate) mod list;
pub(crate) mod dialog;

pub(crate) trait SelectItem<Idx = Option<usize>> {
    fn get_index(&self) -> Idx;

    fn set_index(&mut self, index: Idx);

    fn select_previous(&mut self);

    fn select_next(&mut self);
}
