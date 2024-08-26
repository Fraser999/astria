use anyhow::{
    Context as _,
    Result,
};
use async_trait::async_trait;
use tendermint::Time;
use tracing::instrument;

use crate::storage::{
    BlockHeight,
    BlockTimestamp,
    ChainId,
    RevisionNumber,
    StateRead,
    StateWrite,
    StorageVersion,
};

pub(crate) const CHAIN_ID_KEY: &str = "chain_id";
pub(crate) const REVISION_NUMBER_KEY: &str = "revision_number";
pub(crate) const BLOCK_HEIGHT_KEY: &str = "block_height";
pub(crate) const BLOCK_TIMESTAMP_KEY: &str = "block_timestamp";

fn storage_version_by_height_key(height: u64) -> Vec<u8> {
    format!("storage_version/{height}").into()
}

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_chain_id(&self) -> Result<tendermint::chain::Id> {
        tendermint::chain::Id::try_from(
            self.get::<_, ChainId>(CHAIN_ID_KEY)
                .await
                .transpose()
                .context("chain id not found in state")?
                .context("failed to read chain_id from state")?
                .0,
        )
        .context("invalid chain id from state")
    }

    #[instrument(skip_all)]
    async fn get_revision_number(&self) -> Result<u64> {
        Ok(self
            .get::<_, RevisionNumber>(REVISION_NUMBER_KEY)
            .await
            .context("failed to read revision number from state")?
            .context("revision number not found in state")?
            .0)
    }

    #[instrument(skip_all)]
    async fn get_block_height(&self) -> Result<u64> {
        Ok(self
            .get::<_, BlockHeight>(BLOCK_HEIGHT_KEY)
            .await
            .context("failed to read block_height from state")?
            .context("block height not found in state")?
            .0)
    }

    #[instrument(skip_all)]
    async fn get_block_timestamp(&self) -> Result<Time> {
        Ok(self
            .get::<_, BlockTimestamp>(BLOCK_TIMESTAMP_KEY)
            .await
            .context("failed to read block_timestamp from state")?
            .context("block timestamp not found")?
            .0)
    }

    #[instrument(skip_all)]
    async fn get_storage_version_by_height(&self, height: u64) -> Result<u64> {
        Ok(self
            .nonverifiable_get::<_, StorageVersion>(storage_version_by_height_key(height))
            .await
            .context("failed to read storage_version from state")?
            .context("storage version not found")?
            .0)
    }
}

impl<T: StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_chain_id_and_revision_number(&self, chain_id: tendermint::chain::Id) {
        let revision_number = revision_number_from_chain_id(chain_id.as_str());
        self.put(CHAIN_ID_KEY, ChainId(chain_id.to_string()));
        self.put(REVISION_NUMBER_KEY, RevisionNumber(revision_number));
    }

    #[instrument(skip_all)]
    fn put_block_height(&self, height: u64) {
        self.put(BLOCK_HEIGHT_KEY, BlockHeight(height));
    }

    #[instrument(skip_all)]
    fn put_block_timestamp(&self, timestamp: Time) {
        self.put(BLOCK_TIMESTAMP_KEY, BlockTimestamp(timestamp));
    }

    #[instrument(skip_all)]
    fn put_storage_version_by_height(&self, height: u64, version: u64) {
        self.nonverifiable_put(
            storage_version_by_height_key(height),
            StorageVersion(version),
        );
    }
}

impl<T: StateWrite> StateWriteExt for T {}

fn revision_number_from_chain_id(chain_id: &str) -> u64 {
    let re = regex::Regex::new(r".*-([0-9]+)$").unwrap();

    if !re.is_match(chain_id) {
        tracing::debug!("no revision number found in chain id; setting to 0");
        return 0;
    }

    let (_, revision_number): (&str, [&str; 1]) = re
        .captures(chain_id)
        .expect("should have a matching string")
        .extract();
    revision_number[0]
        .parse::<u64>()
        .expect("revision number must be parseable and fit in a u64")
}

#[cfg(test)]
mod tests {
    use tendermint::Time;

    use super::{
        revision_number_from_chain_id,
        StateReadExt as _,
        StateWriteExt as _,
    };
    use crate::storage::Storage;

    #[test]
    fn revision_number_from_chain_id_regex() {
        let revision_number = revision_number_from_chain_id("test-chain-1024-99");
        assert_eq!(revision_number, 99u64);

        let revision_number = revision_number_from_chain_id("test-chain-1024");
        assert_eq!(revision_number, 1024u64);

        let revision_number = revision_number_from_chain_id("test-chain");
        assert_eq!(revision_number, 0u64);

        let revision_number = revision_number_from_chain_id("99");
        assert_eq!(revision_number, 0u64);

        let revision_number = revision_number_from_chain_id("99-1024");
        assert_eq!(revision_number, 1024u64);

        let revision_number = revision_number_from_chain_id("test-chain-1024-99-");
        assert_eq!(revision_number, 0u64);
    }

    #[tokio::test]
    async fn put_chain_id_and_revision_number() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // doesn't exist at first
        state
            .get_chain_id()
            .await
            .expect_err("no chain ID should exist at first");

        // can write new
        let chain_id_orig: tendermint::chain::Id = "test-chain-orig".try_into().unwrap();
        state.put_chain_id_and_revision_number(chain_id_orig.clone());
        assert_eq!(
            state
                .get_chain_id()
                .await
                .expect("a chain ID was written and must exist inside the database"),
            chain_id_orig,
            "stored chain ID was not what was expected"
        );

        assert_eq!(
            state
                .get_revision_number()
                .await
                .expect("getting the revision number should succeed"),
            0u64,
            "returned revision number should be 0u64 as chain id did not have a revision number"
        );

        // can rewrite with new value
        let chain_id_update: tendermint::chain::Id = "test-chain-update".try_into().unwrap();
        state.put_chain_id_and_revision_number(chain_id_update.clone());
        assert_eq!(
            state
                .get_chain_id()
                .await
                .expect("a new chain ID was written and must exist inside the database"),
            chain_id_update,
            "updated chain ID was not what was expected"
        );

        assert_eq!(
            state
                .get_revision_number()
                .await
                .expect("getting the revision number should succeed"),
            0u64,
            "returned revision number should be 0u64 as chain id did not have a revision number"
        );

        // can rewrite with chain id with revision number
        let chain_id_update: tendermint::chain::Id = "test-chain-99".try_into().unwrap();
        state.put_chain_id_and_revision_number(chain_id_update.clone());
        assert_eq!(
            state
                .get_chain_id()
                .await
                .expect("a new chain ID was written and must exist inside the database"),
            chain_id_update,
            "updated chain ID was not what was expected"
        );

        assert_eq!(
            state
                .get_revision_number()
                .await
                .expect("getting the revision number should succeed"),
            99u64,
            "returned revision number should be 0u64 as chain id did not have a revision number"
        );
    }

    #[tokio::test]
    async fn block_height() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // doesn't exist at first
        state
            .get_block_height()
            .await
            .expect_err("no block height should exist at first");

        // can write new
        let block_height_orig = 0;
        state.put_block_height(block_height_orig);
        assert_eq!(
            state
                .get_block_height()
                .await
                .expect("a block height was written and must exist inside the database"),
            block_height_orig,
            "stored block height was not what was expected"
        );

        // can rewrite with new value
        let block_height_update = 1;
        state.put_block_height(block_height_update);
        assert_eq!(
            state
                .get_block_height()
                .await
                .expect("a new block height was written and must exist inside the database"),
            block_height_update,
            "updated block height was not what was expected"
        );
    }

    #[tokio::test]
    async fn block_timestamp() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // doesn't exist at first
        state
            .get_block_timestamp()
            .await
            .expect_err("no block timestamp should exist at first");

        // can write new
        let block_timestamp_orig = Time::from_unix_timestamp(1_577_836_800, 0).unwrap();
        state.put_block_timestamp(block_timestamp_orig);
        assert_eq!(
            state
                .get_block_timestamp()
                .await
                .expect("a block timestamp was written and must exist inside the database"),
            block_timestamp_orig,
            "stored block timestamp was not what was expected"
        );

        // can rewrite with new value
        let block_timestamp_update = Time::from_unix_timestamp(1_577_836_801, 0).unwrap();
        state.put_block_timestamp(block_timestamp_update);
        assert_eq!(
            state
                .get_block_timestamp()
                .await
                .expect("a new block timestamp was written and must exist inside the database"),
            block_timestamp_update,
            "updated block timestamp was not what was expected"
        );
    }

    #[tokio::test]
    async fn storage_version() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // doesn't exist at first
        let block_height_orig = 0;
        state
            .get_storage_version_by_height(block_height_orig)
            .await
            .expect_err("no block height should exist at first");

        // can write for block height 0
        let storage_version_orig = 0;
        state.put_storage_version_by_height(block_height_orig, storage_version_orig);
        assert_eq!(
            state
                .get_storage_version_by_height(block_height_orig)
                .await
                .expect("a storage version was written and must exist inside the database"),
            storage_version_orig,
            "stored storage version was not what was expected"
        );

        // can update block height 0
        let storage_version_update = 0;
        state.put_storage_version_by_height(block_height_orig, storage_version_update);
        assert_eq!(
            state
                .get_storage_version_by_height(block_height_orig)
                .await
                .expect("a new storage version was written and must exist inside the database"),
            storage_version_update,
            "updated storage version was not what was expected"
        );

        // can write block 1 and block 0 is unchanged
        let block_height_update = 1;
        state.put_storage_version_by_height(block_height_update, storage_version_orig);
        assert_eq!(
            state
                .get_storage_version_by_height(block_height_update)
                .await
                .expect("a second storage version was written and must exist inside the database"),
            storage_version_orig,
            "additional storage version was not what was expected"
        );
        assert_eq!(
            state
                .get_storage_version_by_height(block_height_orig)
                .await
                .expect(
                    "the first storage version was written and should still exist inside the \
                     database"
                ),
            storage_version_update,
            "original but updated storage version was not what was expected"
        );
    }
}
