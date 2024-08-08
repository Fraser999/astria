use anyhow::Result;
use cnidarium::RootHash;

use super::Cache;

#[derive(Clone)]
pub(crate) struct Snapshot {
    inner: cnidarium::Snapshot,
    cache: Cache,
}

impl Snapshot {
    pub(crate) async fn root_hash(&self) -> Result<RootHash> {
        self.inner.root_hash().await
    }

    pub(super) fn new(inner: cnidarium::Snapshot, cache: Cache) -> Self {
        Self {
            inner,
            cache,
        }
    }

    pub(crate) fn inner(&self) -> &cnidarium::Snapshot {
        &self.inner
    }

    pub(crate) fn cache(&self) -> &Cache {
        &self.cache
    }
}
