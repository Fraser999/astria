#[derive(Clone, Debug)]
pub(crate) enum Cached {
    Deleted,
    BlockHeight(u64),
}
