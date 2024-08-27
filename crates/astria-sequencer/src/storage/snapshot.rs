use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use cnidarium::RootHash;
use futures::{
    Stream,
    TryFutureExt,
};
use quick_cache::sync::Cache as QuickCache;

use super::{
    CachedValue,
    SnapshotDelta,
    SnapshotDeltaCompat,
    StateRead,
    Storable,
    StoredValue,
};

#[derive(Clone)]
pub(crate) struct Snapshot {
    /// The underlying snapshot of the DB.
    inner: cnidarium::Snapshot,
    /// An in-memory cache of objects which belong in the verifiable store.
    verifiable_cache: Arc<QuickCache<String, CachedValue>>,
    /// An in-memory cache of objects which belong in the non-verifiable store.
    nonverifiable_cache: Arc<QuickCache<Vec<u8>, CachedValue>>,
}

impl Snapshot {
    pub(super) fn new(inner: cnidarium::Snapshot) -> Self {
        Self {
            inner,
            verifiable_cache: Arc::new(QuickCache::new(500_000)),
            nonverifiable_cache: Arc::new(QuickCache::new(500_000)),
        }
    }

    pub(super) fn inner(&self) -> cnidarium::Snapshot {
        self.inner.clone()
    }

    pub(crate) fn new_delta(&self) -> SnapshotDelta {
        SnapshotDelta::new(self.clone())
    }

    pub(crate) fn new_cnidarium_delta(&self) -> SnapshotDeltaCompat {
        SnapshotDeltaCompat::new(self.inner())
    }

    pub(crate) async fn root_hash(&self) -> Result<RootHash> {
        self.inner.root_hash().await
    }

    pub(crate) fn prefix_keys<K>(
        &self,
        prefix: K,
    ) -> impl Stream<Item = Result<String>> + Send + 'static
    where
        K: AsRef<str>,
    {
        super::prefix_keys(&self.inner, prefix)
    }
}

#[async_trait]
impl StateRead for Snapshot {
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str> + Send,
        V: Storable,
    {
        self.verifiable_cache
            .get_or_insert_async(
                key.as_ref(),
                super::get(&self.inner, key.as_ref()).ok_into::<CachedValue>(),
            )
            .ok_into::<Option<StoredValue>>()
            .await?
            .map(V::try_from)
            .transpose()
    }

    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + Send,
        V: Storable,
    {
        self.nonverifiable_cache
            .get_or_insert_async(
                key.as_ref(),
                super::nonverifiable_get(&self.inner, key.as_ref()).ok_into::<CachedValue>(),
            )
            .ok_into::<Option<StoredValue>>()
            .await?
            .map(V::try_from)
            .transpose()
    }
}
