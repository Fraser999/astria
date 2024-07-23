use anyhow::{
    anyhow,
    bail,
    Context as _,
    Result,
};
use astria_core::primitive::v1::asset;
use async_trait::async_trait;
use futures::{
    StreamExt as _,
    TryStreamExt as _,
};
use tendermint::Time;
use tracing::instrument;

use crate::{
    storage::{
        display_nonverifiable_key,
        StateRead,
        StateWrite,
    },
    storage_keys::hunks::Asset,
};

const CHAIN_ID_KEY: &str = "chain_id";
const REVISION_NUMBER_KEY: &str = "revision_number";
const BLOCK_HEIGHT_KEY: &str = "block_height";
const BLOCK_TIMESTAMP_KEY: &str = "block_timestamp";
const NATIVE_ASSET_KEY: &str = "native_asset";
const BLOCK_FEES_PREFIX: &str = "block_fees/";
const FEE_ASSET_PREFIX: &str = "fee_asset/";

type StoredChainId = String;
type StoredRevisionNumber = u64;
type StoredBlockHeight = u64;
type StoredTimestamp = i128;
type StoredStorageVersion = u64;
type StoredBlockFees = u128;

fn storage_version_by_height_key(height: u64) -> String {
    format!("storage_version/{height}")
}

fn block_fees_key<TAsset: Into<asset::IbcPrefixed>>(asset: TAsset) -> String {
    format!(
        "{BLOCK_FEES_PREFIX}{}",
        crate::storage_keys::hunks::Asset::from(asset)
    )
}

fn fee_asset_key<TAsset: Into<asset::IbcPrefixed>>(asset: TAsset) -> String {
    format!(
        "{FEE_ASSET_PREFIX}{}",
        crate::storage_keys::hunks::Asset::from(asset)
    )
}

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_chain_id(&self) -> Result<tendermint::chain::Id> {
        let Some(chain_id) = self.get::<_, StoredChainId>(CHAIN_ID_KEY).await? else {
            bail!("chain id not found in state");
        };
        Ok(chain_id
            .try_into()
            .expect("only valid chain ids should be stored in the state"))
    }

    #[instrument(skip_all)]
    async fn get_revision_number(&self) -> Result<StoredRevisionNumber> {
        self.get(REVISION_NUMBER_KEY)
            .await?
            .ok_or_else(|| anyhow!("revision number not found in state"))
    }

    #[instrument(skip_all)]
    async fn get_block_height(&self) -> Result<StoredBlockHeight> {
        self.get(BLOCK_HEIGHT_KEY)
            .await?
            .ok_or_else(|| anyhow!("block height not found in state"))
    }

    #[instrument(skip_all)]
    async fn get_block_timestamp(&self) -> Result<Time> {
        const BILLION: i128 = 1_000_000_000;
        let Some(stored_timestamp) = self.get::<_, StoredTimestamp>(BLOCK_TIMESTAMP_KEY).await?
        else {
            bail!("block timestamp not found");
        };
        let seconds = i64::try_from(stored_timestamp / BILLION)
            .unwrap_or_else(|_| panic!("invalid stored block time `{stored_timestamp}`"));
        let nanoseconds = u32::try_from(stored_timestamp % BILLION)
            .unwrap_or_else(|_| panic!("invalid stored block time `{stored_timestamp}`"));
        let timestamp = Time::from_unix_timestamp(seconds, nanoseconds)
            .unwrap_or_else(|_| panic!("invalid stored block time `{stored_timestamp}`"));
        Ok(timestamp)
    }

    #[instrument(skip_all)]
    async fn get_storage_version_by_height(&self, height: u64) -> Result<StoredStorageVersion> {
        self.get(storage_version_by_height_key(height))
            .await?
            .ok_or_else(|| anyhow!("storage version not found in state"))
    }

    #[instrument(skip_all)]
    async fn get_native_asset_denom(&self) -> Result<String> {
        self.get(NATIVE_ASSET_KEY)
            .await?
            .ok_or_else(|| anyhow!("native asset denom not found in state"))
    }

    #[instrument(skip_all)]
    async fn get_block_fees(&self) -> Result<Vec<(asset::IbcPrefixed, StoredBlockFees)>> {
        self.nonverifiable_prefix::<_, StoredBlockFees>(BLOCK_FEES_PREFIX)
            .and_then(|(key, fees)| async move {
                let asset = asset_from_prefixed_key(BLOCK_FEES_PREFIX, &key)?;
                Ok((asset, fees))
            })
            .try_collect()
            .await
    }

    #[instrument(skip_all)]
    async fn is_allowed_fee_asset<TAsset>(&self, asset: TAsset) -> Result<bool>
    where
        TAsset: Into<asset::IbcPrefixed>,
    {
        Ok(self
            .nonverifiable_get::<_, ()>(fee_asset_key(asset))
            .await?
            .is_some())
    }

    #[instrument(skip_all)]
    async fn get_allowed_fee_assets(&self) -> Result<Vec<asset::IbcPrefixed>> {
        self.nonverifiable_prefix::<_, ()>(FEE_ASSET_PREFIX)
            .and_then(|(key, _)| async move { asset_from_prefixed_key(FEE_ASSET_PREFIX, &key) })
            .try_collect()
            .await
    }
}

impl<T: StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_chain_id_and_revision_number(&mut self, chain_id: tendermint::chain::Id) -> Result<()> {
        let stored_revision_number: StoredRevisionNumber =
            revision_number_from_chain_id(chain_id.as_str());
        let stored_chain_id: StoredChainId = chain_id.as_str().to_string();
        self.put(CHAIN_ID_KEY, &stored_chain_id)?;
        self.put(REVISION_NUMBER_KEY, &stored_revision_number)
    }

    #[instrument(skip_all)]
    fn put_block_height(&mut self, height: StoredBlockHeight) -> Result<()> {
        self.put(BLOCK_HEIGHT_KEY, &height)
    }

    #[instrument(skip_all)]
    fn put_block_timestamp(&mut self, timestamp: Time) -> Result<()> {
        self.put(BLOCK_TIMESTAMP_KEY, &timestamp.unix_timestamp_nanos())
    }

    #[instrument(skip_all)]
    fn put_storage_version_by_height(
        &mut self,
        height: u64,
        version: StoredStorageVersion,
    ) -> Result<()> {
        self.nonverifiable_put(storage_version_by_height_key(height), &version)
    }

    #[instrument(skip_all)]
    fn put_native_asset_denom(&mut self, denom: &str) -> Result<()> {
        self.nonverifiable_put(NATIVE_ASSET_KEY, &denom)
    }

    /// Adds `amount` to the block fees for `asset`.
    #[instrument(skip_all)]
    async fn get_and_increase_block_fees<TAsset>(
        &mut self,
        asset: TAsset,
        amount: StoredBlockFees,
    ) -> Result<()>
    where
        TAsset: Into<asset::IbcPrefixed> + std::fmt::Display + Send,
    {
        let block_fees_key = block_fees_key(asset);
        let current_amount: StoredBlockFees = self
            .nonverifiable_get(&block_fees_key)
            .await?
            .unwrap_or_default();

        let new_amount = current_amount
            .checked_add(amount)
            .context("block fees overflowed u128")?;

        self.nonverifiable_put(block_fees_key, &new_amount)
    }

    #[instrument(skip_all)]
    async fn clear_block_fees(&mut self) {
        let mut stream = self.nonverifiable_prefix(BLOCK_FEES_PREFIX);
        while let Some(Ok((key, _))) = stream.next().await {
            self.nonverifiable_delete(key);
        }
    }

    #[instrument(skip_all)]
    fn put_allowed_fee_asset<TAsset>(&mut self, asset: TAsset) -> Result<()>
    where
        TAsset: Into<asset::IbcPrefixed>,
    {
        self.nonverifiable_put(fee_asset_key(asset), &())
    }

    #[instrument(skip_all)]
    fn delete_allowed_fee_asset<TAsset>(&mut self, asset: TAsset)
    where
        TAsset: Into<asset::IbcPrefixed> + std::fmt::Display,
    {
        self.nonverifiable_delete(fee_asset_key(asset));
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

fn asset_from_prefixed_key(prefix: &str, key: &[u8]) -> Result<asset::IbcPrefixed> {
    let suffix = key.strip_prefix(prefix.as_bytes()).with_context(|| {
        format!(
            "expected storage key `{}` to have prefix `{prefix}`",
            display_nonverifiable_key(key)
        )
    })?;
    let asset = std::str::from_utf8(suffix)
        .with_context(|| {
            format!(
                "expected storage key `{}` with suffix `{}` to be UTF-8 encoded",
                display_nonverifiable_key(key),
                display_nonverifiable_key(suffix)
            )
        })?
        .parse::<Asset>()
        .with_context(|| {
            format!(
                "failed to parse storage key `{}` with suffix `{}` as address hunk",
                display_nonverifiable_key(key),
                display_nonverifiable_key(suffix)
            )
        })?
        .get();
    Ok(asset)
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use cnidarium::StateDelta;
    use tendermint::Time;

    use super::{
        revision_number_from_chain_id,
        StateReadExt as _,
        StateWriteExt as _,
    };
    use crate::state_ext::{
        block_fees_key,
        fee_asset_key,
    };

    fn asset_0() -> astria_core::primitive::v1::asset::Denom {
        "asset_0".parse().unwrap()
    }
    fn asset_1() -> astria_core::primitive::v1::asset::Denom {
        "asset_1".parse().unwrap()
    }
    fn asset_2() -> astria_core::primitive::v1::asset::Denom {
        "asset_2".parse().unwrap()
    }

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
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

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
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

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
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

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
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

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

    #[tokio::test]
    async fn native_asset_denom() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        // doesn't exist at first
        state
            .get_native_asset_denom()
            .await
            .expect_err("no native asset denom should exist at first");

        // can write
        let denom_orig = "denom_orig";
        state.put_native_asset_denom(denom_orig);
        assert_eq!(
            state.get_native_asset_denom().await.expect(
                "a native asset denomination was written and must exist inside the database"
            ),
            denom_orig,
            "stored native asset denomination was not what was expected"
        );

        // can write new value
        let denom_update = "denom_update";
        state.put_native_asset_denom(denom_update);
        assert_eq!(
            state.get_native_asset_denom().await.expect(
                "a native asset denomination update was written and must exist inside the database"
            ),
            denom_update,
            "updated native asset denomination was not what was expected"
        );
    }

    #[tokio::test]
    async fn block_fee_read_and_increase() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        // doesn't exist at first
        let fee_balances_orig = state.get_block_fees().await.unwrap();
        assert!(fee_balances_orig.is_empty());

        // can write
        let asset = asset_0();
        let amount = 100u128;
        state
            .get_and_increase_block_fees(&asset, amount)
            .await
            .unwrap();

        // holds expected
        let fee_balances_updated = state.get_block_fees().await.unwrap();
        assert_eq!(
            fee_balances_updated[0],
            (asset.to_ibc_prefixed(), amount),
            "fee balances are not what they were expected to be"
        );
    }

    #[tokio::test]
    async fn block_fee_read_and_increase_can_delete() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        // can write
        let asset_first = asset_0();
        let asset_second = asset_1();
        let amount_first = 100u128;
        let amount_second = 200u128;

        state
            .get_and_increase_block_fees(&asset_first, amount_first)
            .await
            .unwrap();
        state
            .get_and_increase_block_fees(&asset_second, amount_second)
            .await
            .unwrap();
        // holds expected
        let fee_balances = HashSet::<_>::from_iter(state.get_block_fees().await.unwrap());
        assert_eq!(
            fee_balances,
            HashSet::from_iter(vec![
                (asset_first.to_ibc_prefixed(), amount_first),
                (asset_second.to_ibc_prefixed(), amount_second)
            ]),
            "returned fee balance vector not what was expected"
        );

        // can delete
        state.clear_block_fees().await;

        let fee_balances_updated = state.get_block_fees().await.unwrap();
        assert!(
            fee_balances_updated.is_empty(),
            "fee balances were expected to be deleted but were not"
        );
    }

    #[tokio::test]
    async fn is_allowed_fee_asset() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        // non-existent fees assets return false
        let asset = asset_0();
        assert!(
            !state
                .is_allowed_fee_asset(&asset)
                .await
                .expect("checking for allowed fee asset should not fail"),
            "fee asset was expected to return false"
        );

        // existent fee assets return true
        state.put_allowed_fee_asset(&asset);
        assert!(
            state
                .is_allowed_fee_asset(&asset)
                .await
                .expect("checking for allowed fee asset should not fail"),
            "fee asset was expected to be allowed"
        );
    }

    #[tokio::test]
    async fn can_delete_allowed_fee_assets_simple() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        // setup fee asset
        let asset = asset_0();
        state.put_allowed_fee_asset(&asset);
        assert!(
            state
                .is_allowed_fee_asset(&asset)
                .await
                .expect("checking for allowed fee asset should not fail"),
            "fee asset was expected to be allowed"
        );

        // see can get fee asset
        let assets = state.get_allowed_fee_assets().await.unwrap();
        assert_eq!(
            assets,
            vec![asset.to_ibc_prefixed()],
            "expected returned allowed fee assets to match what was written in"
        );

        // can delete
        state.delete_allowed_fee_asset(&asset);

        // see is deleted
        let assets = state.get_allowed_fee_assets().await.unwrap();
        assert!(assets.is_empty(), "fee assets should be empty post delete");
    }

    #[tokio::test]
    async fn can_delete_allowed_fee_assets_complex() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        // setup fee assets
        let asset_first = asset_0();
        state.put_allowed_fee_asset(&asset_first);
        assert!(
            state
                .is_allowed_fee_asset(&asset_first)
                .await
                .expect("checking for allowed fee asset should not fail"),
            "fee asset was expected to be allowed"
        );
        let asset_second = asset_1();
        state.put_allowed_fee_asset(&asset_second);
        assert!(
            state
                .is_allowed_fee_asset(&asset_second)
                .await
                .expect("checking for allowed fee asset should not fail"),
            "fee asset was expected to be allowed"
        );
        let asset_third = asset_2();
        state.put_allowed_fee_asset(&asset_third);
        assert!(
            state
                .is_allowed_fee_asset(&asset_third)
                .await
                .expect("checking for allowed fee asset should not fail"),
            "fee asset was expected to be allowed"
        );

        // can delete
        state.delete_allowed_fee_asset(&asset_second);

        // see is deleted
        let assets = HashSet::<_>::from_iter(state.get_allowed_fee_assets().await.unwrap());
        assert_eq!(
            assets,
            HashSet::from_iter(vec![
                asset_first.to_ibc_prefixed(),
                asset_third.to_ibc_prefixed()
            ]),
            "delete for allowed fee asset did not behave as expected"
        );
    }

    #[test]
    fn storage_keys_are_not_changed() {
        let trace_prefixed = "a/denom/with/a/prefix"
            .parse::<astria_core::primitive::v1::asset::Denom>()
            .unwrap();
        assert_eq!(
            block_fees_key(&trace_prefixed),
            block_fees_key(trace_prefixed.to_ibc_prefixed()),
        );
        insta::assert_snapshot!(block_fees_key(&trace_prefixed));

        assert_eq!(
            fee_asset_key(&trace_prefixed),
            fee_asset_key(trace_prefixed.to_ibc_prefixed()),
        );
        insta::assert_snapshot!(fee_asset_key(trace_prefixed));
    }
}
