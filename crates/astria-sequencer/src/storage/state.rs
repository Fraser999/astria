use std::collections::{
    BTreeMap,
    HashMap,
};

use anyhow::Result;
use astria_core::{
    primitive::v1::{
        asset,
        RollupId,
    },
    sequencerblock::v1alpha1::block::Deposit,
};
use async_trait::async_trait;
use tendermint::abci;

use super::Storable;

#[async_trait]
pub(crate) trait StateRead: Send + Sync {
    /// Gets a value from the verifiable key-value store.
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str> + Send,
        V: Storable;

    /// Gets a value from the non-verifiable key-value store.
    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + Send,
        V: Storable;
}

#[async_trait]
impl<'a, S: StateRead> StateRead for &'a S {
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str> + Send,
        V: Storable,
    {
        (**self).get(key).await
    }

    /// Gets a value from the non-verifiable key-value store.
    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + Send,
        V: Storable,
    {
        (**self).nonverifiable_get(key).await
    }
}

pub(crate) trait StateWrite: StateRead + Send + Sync {
    /// Puts the given value into the verifiable key-value store under the given key.
    fn put<K, V>(&self, key: K, value: V)
    where
        K: Into<String>,
        V: Storable;

    /// Deletes a key-value from the verifiable key-value store.
    fn delete<K: Into<String>>(&self, key: K);

    /// Puts the given value into the non-verifiable key-value store under the given key.
    fn nonverifiable_put<K, V>(&self, key: K, value: V)
    where
        K: Into<Vec<u8>>,
        V: Storable;

    /// Deletes a key-value from the non-verifiable key-value store.
    fn nonverifiable_delete<K: Into<Vec<u8>>>(&self, key: K);

    /// Record the given event.
    fn record(&self, event: abci::Event);

    /// Returns a clone of the current block fees.
    fn block_fees(&self) -> BTreeMap<asset::IbcPrefixed, u128>;

    /// Adds `amount` to the block fees for `asset`.
    fn increase_block_fees<TAsset>(&self, asset: TAsset, amount: u128) -> Result<()>
    where
        TAsset: Into<asset::IbcPrefixed>;

    /// Returns a clone of the bridge deposits.
    fn bridge_deposits(&self) -> HashMap<RollupId, Vec<Deposit>>;

    /// Adds the given bridge deposit.
    fn put_bridge_deposit(&self, deposit: Deposit);
}

impl<'a, S: StateWrite + Send + Sync> StateWrite for &'a S {
    fn put<K, V>(&self, key: K, value: V)
    where
        K: Into<String>,
        V: Storable,
    {
        (**self).put(key, value);
    }

    fn delete<K: Into<String>>(&self, key: K) {
        (**self).delete(key);
    }

    fn nonverifiable_put<K, V>(&self, key: K, value: V)
    where
        K: Into<Vec<u8>>,
        V: Storable,
    {
        (**self).nonverifiable_put(key, value);
    }

    fn nonverifiable_delete<K: Into<Vec<u8>>>(&self, key: K) {
        (**self).nonverifiable_delete(key);
    }

    fn record(&self, event: abci::Event) {
        (**self).record(event);
    }

    fn block_fees(&self) -> BTreeMap<asset::IbcPrefixed, u128> {
        (**self).block_fees()
    }

    fn increase_block_fees<TAsset>(&self, asset: TAsset, amount: u128) -> Result<()>
    where
        TAsset: Into<asset::IbcPrefixed>,
    {
        (**self).increase_block_fees(asset, amount)
    }

    fn bridge_deposits(&self) -> HashMap<RollupId, Vec<Deposit>> {
        (**self).bridge_deposits()
    }

    fn put_bridge_deposit(&self, deposit: Deposit) {
        (**self).put_bridge_deposit(deposit);
    }
}
