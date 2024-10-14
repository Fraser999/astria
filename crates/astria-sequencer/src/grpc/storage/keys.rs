use astria_core::{
    primitive::v1::RollupId,
    sequencerblock::v1alpha1::block::SequencerBlockHash,
};

pub(in crate::grpc) fn block_hash_by_height(height: u64) -> String {
    format!("grpc/block_hash/{height}")
}

pub(in crate::grpc) fn sequencer_block_header_by_hash(block_hash: &SequencerBlockHash) -> String {
    format!("grpc/block_header/{block_hash}")
}

pub(in crate::grpc) fn rollup_data_by_hash_and_rollup_id(
    block_hash: &SequencerBlockHash,
    rollup_id: &RollupId,
) -> String {
    format!("grpc/rollup_data/{block_hash}/{rollup_id}",)
}

pub(in crate::grpc) fn rollup_ids_by_hash(block_hash: &SequencerBlockHash) -> String {
    format!("grpc/rollup_ids/{block_hash}")
}

pub(in crate::grpc) fn rollup_transactions_proof_by_hash(
    block_hash: &SequencerBlockHash,
) -> String {
    format!("grpc/rollup_txs_proof/{block_hash}",)
}

pub(in crate::grpc) fn rollup_ids_proof_by_hash(block_hash: &SequencerBlockHash) -> String {
    format!("grpc/rollup_ids_proof/{block_hash}",)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPONENT_PREFIX: &str = "grpc/";
    const BLOCK_HASH: SequencerBlockHash = SequencerBlockHash::new([1; 32]);
    const ROLLUP_ID: RollupId = RollupId::new([2; 32]);

    #[test]
    fn keys_should_not_change() {
        insta::assert_snapshot!(block_hash_by_height(42));
        insta::assert_snapshot!(sequencer_block_header_by_hash(&BLOCK_HASH));
        insta::assert_snapshot!(rollup_data_by_hash_and_rollup_id(&BLOCK_HASH, &ROLLUP_ID));
        insta::assert_snapshot!(rollup_ids_by_hash(&BLOCK_HASH));
        insta::assert_snapshot!(rollup_transactions_proof_by_hash(&BLOCK_HASH));
        insta::assert_snapshot!(rollup_ids_proof_by_hash(&BLOCK_HASH));
    }

    #[test]
    fn keys_should_have_component_prefix() {
        assert!(block_hash_by_height(42).starts_with(COMPONENT_PREFIX));
        assert!(sequencer_block_header_by_hash(&BLOCK_HASH).starts_with(COMPONENT_PREFIX));
        assert!(
            rollup_data_by_hash_and_rollup_id(&BLOCK_HASH, &ROLLUP_ID)
                .starts_with(COMPONENT_PREFIX)
        );
        assert!(rollup_ids_by_hash(&BLOCK_HASH).starts_with(COMPONENT_PREFIX));
        assert!(rollup_transactions_proof_by_hash(&BLOCK_HASH).starts_with(COMPONENT_PREFIX));
        assert!(rollup_ids_proof_by_hash(&BLOCK_HASH).starts_with(COMPONENT_PREFIX));
    }
}
