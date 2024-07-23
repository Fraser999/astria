use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};
use futures::Stream;

use super::{
    DeltaDelta,
    Snapshot,
    StateRead,
    StateWrite,
};

pub(crate) struct SnapshotDelta {
    inner: Arc<cnidarium::StateDelta<cnidarium::Snapshot>>,
}

impl SnapshotDelta {
    pub(crate) fn new(snapshot: Snapshot) -> Self {
        Self {
            inner: Arc::new(cnidarium::StateDelta::new(snapshot.inner())),
        }
    }

    pub(crate) fn try_begin_transaction(&mut self) -> Option<DeltaDelta> {
        Arc::get_mut(&mut self.inner).map(DeltaDelta::new)
    }

    pub(super) fn inner(self) -> Arc<cnidarium::StateDelta<cnidarium::Snapshot>> {
        self.inner
    }

    pub(crate) fn inner_mut(&mut self) -> &mut cnidarium::StateDelta<cnidarium::Snapshot> {
        &mut self.inner
    }
}

#[async_trait]
impl StateRead for SnapshotDelta {
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

impl StateWrite for SnapshotDelta {
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
