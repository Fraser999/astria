mod cache;
mod cached;
mod delta_delta;
mod snapshot;
mod snapshot_delta;

use std::path::PathBuf;

use anyhow::Result;
use cnidarium::{
    RootHash,
    StagedWriteBatch,
};

pub(crate) use self::{
    cache::Cache,
    cached::Cached,
    delta_delta::DeltaDelta,
    snapshot::Snapshot,
    snapshot_delta::SnapshotDelta,
};

#[derive(Clone, Debug)]
pub(crate) struct Storage {
    inner: cnidarium::Storage,
    cache: Cache,
    #[cfg(test)]
    _temp_dir: Option<std::sync::Arc<tempfile::TempDir>>,
}

impl Storage {
    pub(crate) async fn init(path: PathBuf, prefixes: Vec<String>) -> Result<Self> {
        let inner = cnidarium::Storage::init(path, prefixes).await?;
        Ok(Self {
            inner,
            cache: Cache::new(),
            #[cfg(test)]
            _temp_dir: None,
        })
    }

    #[cfg(test)]
    pub(crate) async fn new_temp() -> Self {
        let temp_dir = tempfile::tempdir().unwrap_or_else(|error| {
            panic!("failed to create temp dir when constructing storage instance: {error}")
        });
        let db_path = temp_dir.path().join("storage.db");
        let inner = cnidarium::Storage::init(db_path.clone(), vec![])
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "failed to initialize storage at `{}`: {error:#}",
                    db_path.display()
                )
            });

        Self {
            inner,
            cache: Cache::new(),
            _temp_dir: Some(std::sync::Arc::new(temp_dir)),
        }
    }

    /// Returns the latest version (block height) of the tree recorded by `Storage`.
    ///
    /// If the tree is empty and has not been initialized, returns `u64::MAX`.
    pub fn latest_version(&self) -> u64 {
        self.inner.latest_version()
    }

    /// Returns a new [`Snapshot`] on top of the latest version of the tree.
    pub fn latest_snapshot(&self) -> Snapshot {
        Snapshot::new(self.inner.latest_snapshot(), self.cache.clone())
    }

    /// Returns the [`Snapshot`] corresponding to the given version.
    pub fn snapshot(&self, version: u64) -> Option<Snapshot> {
        Some(Snapshot::new(self.inner.snapshot(version)?, Cache::new()))
    }

    /// Returns a new [`SnapshotDelta`] on top of the latest version of the tree.
    pub fn latest_snapshot_delta(&self) -> SnapshotDelta {
        SnapshotDelta::new(self.latest_snapshot())
    }

    /// Prepares a commit for the provided [`SnapshotDelta`], returning a [`StagedWriteBatch`].
    ///
    /// The batch can be committed to the database using the [`Storage::commit_batch`] method.
    pub async fn prepare_commit(&self, delta: SnapshotDelta) -> Result<(StagedWriteBatch, Cache)> {
        let batch = self
            .inner
            .prepare_commit(std::sync::Arc::into_inner(delta.inner()).unwrap())
            .await?;
        // todo(Fraser): avoid cloning
        let cache = delta.cache().clone().apply_changes().await;
        Ok((batch, cache))
    }

    /// Commits the provided [`StateDelta`] to persistent storage as the latest version of the chain
    /// state.
    pub async fn commit(&self, delta: SnapshotDelta) -> Result<(RootHash, Cache)> {
        let root_hash = self
            .inner
            .commit(std::sync::Arc::into_inner(delta.inner()).unwrap())
            .await?;
        // todo(Fraser): avoid cloning
        let cache = delta.cache().clone().apply_changes().await;
        Ok((root_hash, cache))
    }

    /// Commits the supplied [`StagedWriteBatch`] to persistent storage.
    pub fn commit_batch(&self, batch: StagedWriteBatch) -> Result<RootHash> {
        self.inner.commit_batch(batch)
    }
}
