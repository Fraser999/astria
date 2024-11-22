pub mod block;
pub mod celestia;

pub use block::{
    DataItem,
    RollupTransactions,
    SequencerBlock,
};
pub use celestia::{
    SubmittedMetadata,
    SubmittedRollupData,
};
use indexmap::IndexMap;
use sha2::{
    Digest as _,
    Sha256,
};

use crate::{
    generated::astria::sequencerblock::v1 as raw,
    primitive::v1::{
        derive_merkle_tree_from_rollup_txs,
        IncorrectRollupIdLength,
        RollupId,
    },
};

pub(crate) fn are_rollup_ids_included<TRollupIds>(
    ids: TRollupIds,
    proof: &merkle::Proof,
    data_hash: [u8; 32],
) -> bool
where
    TRollupIds: IntoIterator<Item = RollupId>,
{
    let tree = merkle::Tree::from_leaves(ids);
    let hash_of_root = Sha256::digest(tree.root());
    proof.verify(&hash_of_root, data_hash)
}

pub(crate) fn are_rollup_txs_included(
    rollup_datas: &IndexMap<RollupId, RollupTransactions>,
    rollup_proof: &merkle::Proof,
    data_hash: [u8; 32],
) -> bool {
    let rollup_datas = rollup_datas
        .iter()
        .map(|(rollup_id, tx_data)| (rollup_id, tx_data.transactions()));

    let rollup_tree_root = derive_merkle_tree_from_rollup_txs(rollup_datas).root();
    let data_item = DataItem::RollupIdsRoot(rollup_tree_root);
    let Ok(leaf_hash) = data_item.calculate_hash() else {
        return false;
    };
    rollup_proof.verify(&leaf_hash, data_hash)
}

fn do_rollup_transactions_match_root(
    rollup_transactions: &RollupTransactions,
    root: [u8; 32],
) -> bool {
    let id = rollup_transactions.rollup_id();
    rollup_transactions
        .proof()
        .audit()
        .with_root(root)
        .with_leaf_builder()
        .write(id.as_ref())
        .write(&merkle::Tree::from_leaves(rollup_transactions.transactions()).root())
        .finish_leaf()
        .perform()
}
