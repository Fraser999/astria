use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};
use futures::Stream;
use tendermint::abci::Event;

use super::{
    SnapshotDelta,
    StateRead,
    StateWrite,
};

pub(crate) struct DeltaDelta {
    pub(super) inner: cnidarium::StateDelta<Arc<cnidarium::StateDelta<cnidarium::Snapshot>>>,
}

impl DeltaDelta {
    pub(super) fn new(snapshot_delta: Arc<SnapshotDelta>) -> Self {
        Self {
            inner: cnidarium::StateDelta::new(snapshot_delta.inner_mut()),
        }
    }

    pub(crate) fn into_cache(self) -> cnidarium::Cache {
        self.inner.flatten().1
    }

    pub(crate) fn apply(self) -> Vec<Event> {
        self.inner.apply().1
    }

    // pub(super) fn inner(self) ->
    // cnidarium::StateDelta<cnidarium::StateDelta<cnidarium::Snapshot>> {     self.inner
    // }
}

#[async_trait]
impl StateRead for DeltaDelta {
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str>,
        V: BorshDeserialize,
    {
        super::get(&self.inner, key).await
    }

    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize,
    {
        super::nonverifiable_get(&self.inner, key).await
    }

    fn prefix_keys<K>(&self, prefix: K) -> impl Stream<Item = Result<String>> + Send + 'static
    where
        K: AsRef<str>,
    {
        super::prefix_keys(&self.inner, prefix)
    }

    fn nonverifiable_prefix<K, V>(
        &self,
        prefix: K,
    ) -> impl Stream<Item = Result<(Vec<u8>, V)>> + Send + 'static
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize + Send + 'static,
    {
        super::nonverifiable_prefix(&self.inner, prefix)
    }
}

impl StateWrite for DeltaDelta {
    fn put<K, V>(&mut self, key: K, value: &V) -> Result<()>
    where
        K: Into<String>,
        V: BorshSerialize,
    {
        super::put(&mut self.inner, key, value)
    }

    fn delete<K: Into<String>>(&mut self, key: K) {
        super::delete(&mut self.inner, key)
    }

    fn nonverifiable_put<K, V>(&mut self, key: K, value: &V) -> Result<()>
    where
        K: Into<Vec<u8>>,
        V: BorshSerialize,
    {
        super::nonverifiable_put(&mut self.inner, key, value)
    }

    fn nonverifiable_delete<K>(&mut self, key: K)
    where
        K: Into<Vec<u8>>,
    {
        super::nonverifiable_delete(&mut self.inner, key)
    }
}
