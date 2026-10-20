#[derive(Clone, Copy, Debug)]
pub(crate) struct ArchivedTarget {
    pub(crate) guild_id: u64,
    pub(crate) channel_id: u64,
    pub(crate) parent_id: Option<u64>,
    pub(crate) is_thread: bool,
}
