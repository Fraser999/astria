use anyhow::Result;
use async_trait::async_trait;
use borsh::BorshDeserialize;
use cnidarium::RootHash;
use futures::Stream;

use super::StateRead;

#[derive(Clone)]
pub(crate) struct Snapshot {
    inner: cnidarium::Snapshot,
}

impl Snapshot {
    pub(crate) async fn root_hash(&self) -> Result<RootHash> {
        self.inner.root_hash().await
    }

    pub(super) fn new(inner: cnidarium::Snapshot) -> Self {
        Self {
            inner,
        }
    }

    pub(super) fn inner(self) -> cnidarium::Snapshot {
        self.inner
    }
}

#[async_trait]
impl StateRead for Snapshot {
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
