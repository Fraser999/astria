use astria_core::protocol::transaction::v1::Action;

#[derive(Debug)]
pub(crate) struct Batch {
    /// The withdrawal payloads
    pub(crate) actions: Vec<Action>,
    /// The corresponding rollup block height
    pub(crate) rollup_height: u64,
}
