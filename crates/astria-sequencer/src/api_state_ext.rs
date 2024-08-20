use anyhow::{
    anyhow,
    Context as _,
    Result,
};
use astria_core::{
    primitive::v1::RollupId,
    sequencerblock::v1alpha1::block::{
        SequencerBlock,
        SequencerBlockHeader,
    },
};
use async_trait::async_trait;
use tracing::instrument;

use crate::storage::{
    BlockHash,
    StateRead,
    StateWrite,
};

fn block_hash_by_height_key(height: u64) -> String {
    format!("blockhash/{height}")
}

fn sequencer_block_by_hash_key(hash: &[u8]) -> String {
    format!("block/{}", crate::utils::Hex(hash))
}

fn rollup_ids_by_hash_key(hash: &[u8]) -> String {
    format!("rollupids/{}", crate::utils::Hex(hash))
}

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_block_hash_by_height(&self, height: u64) -> Result<[u8; 32]> {
        Ok(self
            .get::<_, BlockHash>(block_hash_by_height_key(height))
            .await
            .context("failed to read block hash by height from state")?
            .ok_or_else(|| anyhow!("block hash not found for height {height}"))?
            .0)
    }

    #[instrument(skip_all)]
    async fn get_sequencer_block_header_by_hash(
        &self,
        hash: &[u8],
    ) -> Result<SequencerBlockHeader> {
        self.get_sequencer_block_by_hash(hash)
            .await
            .map(|block| block.into_parts().header)
    }

    #[instrument(skip_all)]
    async fn get_rollup_ids_by_block_hash(&self, hash: &[u8]) -> Result<Vec<RollupId>> {
        self.get::<_, Vec<RollupId>>(rollup_ids_by_hash_key(hash))
            .await
            .transpose()
            .context("rollup IDs not found for given block hash")?
            .context("failed to read rollup IDs by block hash from state")
    }

    #[instrument(skip_all)]
    async fn get_sequencer_block_by_hash(&self, hash: &[u8]) -> Result<SequencerBlock> {
        self.get::<_, SequencerBlock>(sequencer_block_by_hash_key(hash))
            .await
            .transpose()
            .context("sequencer block not found for given block hash")?
            .context("failed to read raw sequencer block from state")
    }

    #[instrument(skip_all)]
    async fn get_sequencer_block_by_height(&self, height: u64) -> Result<SequencerBlock> {
        let hash = self
            .get_block_hash_by_height(height)
            .await
            .context("failed to get block hash by height")?;
        self.get_sequencer_block_by_hash(&hash)
            .await
            .context("failed to get sequencer block by hash")
    }
}

impl<T: StateRead> StateReadExt for T {}

pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_sequencer_block(&self, block: SequencerBlock) -> Result<()> {
        let key = block_hash_by_height_key(block.height().into());
        self.put(key, BlockHash(block.block_hash()));

        let rollup_ids = block
            .rollup_transactions()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let key = rollup_ids_by_hash_key(&block.block_hash());
        self.put(key, rollup_ids);

        let key = sequencer_block_by_hash_key(&block.block_hash());
        self.put(key, block);
        Ok(())
    }
}

impl<T: StateWrite> StateWriteExt for T {}

#[cfg(test)]
mod test {
    use astria_core::{
        protocol::test_utils::ConfigureSequencerBlock,
        sequencerblock::v1alpha1::block::Deposit,
    };
    use rand::Rng;

    use super::*;
    use crate::{
        storage::Storage,
        test_utils::astria_address,
    };

    // creates new sequencer block, optionally shifting all values except the height by 1
    fn make_test_sequencer_block(height: u32) -> SequencerBlock {
        let mut rng = rand::thread_rng();
        let block_hash: [u8; 32] = rng.gen();

        // create inner rollup id/tx data
        let mut deposits = vec![];
        for _ in 0..2 {
            let rollup_id = RollupId::new(rng.gen());
            let bridge_address = astria_address(&[rng.gen(); 20]);
            let amount = rng.gen::<u128>();
            let asset = "testasset".parse().unwrap();
            let destination_chain_address = rng.gen::<u8>().to_string();
            let deposit = Deposit::new(
                bridge_address,
                rollup_id,
                amount,
                asset,
                destination_chain_address,
            );
            deposits.push(deposit);
        }

        ConfigureSequencerBlock {
            block_hash: Some(block_hash),
            height,
            deposits,
            ..Default::default()
        }
        .make()
    }

    #[tokio::test]
    async fn put_sequencer_block() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // can write one
        let block_0 = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block_0.clone())
            .expect("writing block to database should work");

        assert_eq!(
            state
                .get_sequencer_block_by_height(block_0.height().into())
                .await
                .expect("a block was written to the database and should exist"),
            block_0,
            "stored block does not match expected"
        );

        // can write another and both are ok
        let block_1 = make_test_sequencer_block(3u32);
        state
            .put_sequencer_block(block_1.clone())
            .expect("writing another block to database should work");
        assert_eq!(
            state
                .get_sequencer_block_by_height(block_0.height().into())
                .await
                .expect("a block was written to the database and should exist"),
            block_0,
            "original stored block does not match expected"
        );
        assert_eq!(
            state
                .get_sequencer_block_by_height(block_1.height().into())
                .await
                .expect("a block was written to the database and should exist"),
            block_1,
            "additionally stored block does not match expected"
        );
    }

    #[tokio::test]
    async fn put_sequencer_block_update() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // write original block
        let mut block = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block.clone())
            .expect("writing block to database should work");
        assert_eq!(
            state
                .get_sequencer_block_by_height(block.height().into())
                .await
                .expect("a block was written to the database and should exist"),
            block,
            "stored block does not match expected"
        );

        // write to same height but with new values
        block = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block.clone())
            .expect("writing block update to database should work");

        // block was updates
        assert_eq!(
            state
                .get_sequencer_block_by_height(block.height().into())
                .await
                .expect("a block was written to the database and should exist"),
            block,
            "updated stored block does not match expected"
        );
    }

    #[tokio::test]
    async fn get_block_hash_by_height() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // write block
        let block = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block.clone())
            .expect("writing block to database should work");

        // grab block hash by block height
        assert_eq!(
            state
                .get_block_hash_by_height(block.height().into())
                .await
                .expect(
                    "a block was written to the database and we should be able to query its block \
                     hash by height"
                ),
            block.block_hash(),
            "stored block hash does not match expected"
        );
    }

    #[tokio::test]
    async fn get_sequencer_block_header_by_hash() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // write block
        let block = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block.clone())
            .expect("writing block to database should work");

        // grab block header by block hash
        assert_eq!(
            state
                .get_sequencer_block_header_by_hash(block.block_hash().as_ref())
                .await
                .expect(
                    "a block was written to the database and we should be able to query its block \
                     header by block hash"
                ),
            block.header().clone(),
            "stored block header does not match expected"
        );
    }

    #[tokio::test]
    async fn get_rollup_ids_by_block_hash() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // write block
        let block = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block.clone())
            .expect("writing block to database should work");

        // grab rollup ids by block hash
        let stored_rollup_ids = state
            .get_rollup_ids_by_block_hash(block.block_hash().as_ref())
            .await
            .expect(
                "a block was written to the database and we should be able to query its rollup ids",
            );
        let original_rollup_ids: Vec<RollupId> =
            block.rollup_transactions().keys().copied().collect();
        assert_eq!(
            stored_rollup_ids, original_rollup_ids,
            "stored rollup ids do not match expected"
        );
    }

    #[tokio::test]
    async fn get_sequencer_block_by_hash() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // write block
        let block = make_test_sequencer_block(2u32);
        state
            .put_sequencer_block(block.clone())
            .expect("writing block to database should work");

        // grab block by block hash
        assert_eq!(
            state
                .get_sequencer_block_by_hash(block.block_hash().as_ref())
                .await
                .expect(
                    "a block was written to the database and we should be able to query its block \
                     by block hash"
                ),
            block,
            "stored block does not match expected"
        );
    }
}
