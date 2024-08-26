mod cache;
mod cnidarium_compat;
mod delta;
mod snapshot;
mod state;
mod stored;

use std::{
    fmt::{
        self,
        Debug,
        Formatter,
    },
    path::PathBuf,
    sync::{
        Arc,
        Mutex,
    },
};

use anyhow::{
    Context,
    Result,
};
use cnidarium::{
    RootHash,
    StagedWriteBatch,
};
use futures::Stream;

use self::{
    cache::CachedValue,
    delta::DeltaInner,
};
pub(crate) use self::{
    cnidarium_compat::{
        DeltaDeltaCompat,
        SnapshotDeltaCompat,
    },
    delta::{
        DeltaDelta,
        SnapshotDelta,
    },
    snapshot::Snapshot,
    state::{
        StateRead,
        StateWrite,
    },
    stored::{
        AddressBytes,
        Balance,
        BasePrefix,
        BlockHash,
        BlockHeight,
        BlockTimestamp,
        ChainId,
        Fee,
        Nonce,
        RevisionNumber,
        Storable,
        StorageVersion,
        StoredValue,
        TxHash,
    },
};

#[derive(Clone)]
pub(crate) struct Storage {
    inner: cnidarium::Storage,
    latest_snapshot: Arc<Mutex<Snapshot>>,
    #[cfg(any(test, feature = "benchmark"))]
    _temp_dir: Option<Arc<tempfile::TempDir>>,
}

impl Storage {
    pub(crate) async fn load(path: PathBuf, prefixes: Vec<String>) -> Result<Self> {
        let inner = cnidarium::Storage::load(path, prefixes).await?;
        let latest_snapshot = Arc::new(Mutex::new(Snapshot::new(inner.latest_snapshot())));
        Ok(Self {
            inner,
            latest_snapshot,
            #[cfg(any(test, feature = "benchmark"))]
            _temp_dir: None,
        })
    }

    #[cfg(any(test, feature = "benchmark"))]
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
        let latest_snapshot = Arc::new(Mutex::new(Snapshot::new(inner.latest_snapshot())));

        Self {
            inner,
            latest_snapshot,
            _temp_dir: Some(std::sync::Arc::new(temp_dir)),
        }
    }

    /// Returns the latest version (block height) of the tree recorded by `Storage`.
    ///
    /// If the tree is empty and has not been initialized, returns `u64::MAX`.
    pub(crate) fn latest_version(&self) -> u64 {
        self.inner.latest_version()
    }

    /// Returns a new `Snapshot` on top of the latest version of the tree.
    pub(crate) fn latest_snapshot(&self) -> Snapshot {
        self.latest_snapshot.lock().unwrap().clone()
    }

    /// Returns the `Snapshot` corresponding to the given version.
    pub(crate) fn snapshot(&self, version: u64) -> Option<Snapshot> {
        Some(Snapshot::new(self.inner.snapshot(version)?))
    }

    /// Returns a new `SnapshotDelta` on top of the latest version of the tree.
    pub(crate) fn new_delta_of_latest_snapshot(&self) -> SnapshotDelta {
        SnapshotDelta::new(self.latest_snapshot())
    }

    /// Prepares a commit for the provided `SnapshotDelta`, returning a `StagedWriteBatch`.
    ///
    /// The batch can be committed to the database using the [`Storage::commit_batch`] method.
    pub(crate) async fn prepare_commit(
        &self,
        delta: SnapshotDelta,
        cnidarium_delta: SnapshotDeltaCompat,
    ) -> Result<StagedWriteBatch> {
        let DeltaInner {
            verifiable_changes,
            nonverifiable_changes,
            block_fees: _,
            bridge_deposits: _,
            events,
        } = delta
            .consume()
            .context("failed to commit: already committed")?;
        let mut cnidarium_delta = cnidarium_delta.take_inner().unwrap();

        for (key, cached_value) in verifiable_changes {
            match cached_value {
                CachedValue::Absent => delete(&mut cnidarium_delta, key),
                CachedValue::Stored(stored_value) => {
                    put(&mut cnidarium_delta, key, &stored_value)?;
                }
            }
        }

        for (key, cached_value) in nonverifiable_changes {
            match cached_value {
                CachedValue::Absent => nonverifiable_delete(&mut cnidarium_delta, key),
                CachedValue::Stored(stored_value) => {
                    nonverifiable_put(&mut cnidarium_delta, key, &stored_value)?;
                }
            }
        }

        // TODO(Fraser): should these be emitted here instead of...?
        for _event in events {}

        self.inner.prepare_commit(cnidarium_delta).await
    }

    /// Commits the provided `SnapshotDelta` to persistent storage as the latest version of the
    /// chain state.
    #[cfg(test)]
    pub(crate) async fn commit(&self, delta: SnapshotDelta) -> Result<RootHash> {
        // TODO(Fraser) - fix
        let cnidarium_delta = SnapshotDeltaCompat::new(self.latest_snapshot().inner());
        let batch = self.prepare_commit(delta, cnidarium_delta).await?;
        self.commit_batch(batch)
    }

    /// Commits the supplied `StagedWriteBatch` to persistent storage.
    pub(crate) fn commit_batch(&self, batch: StagedWriteBatch) -> Result<RootHash> {
        let root_hash = self.inner.commit_batch(batch)?;
        let mut ls = self.latest_snapshot.lock().unwrap();
        *ls = Snapshot::new(self.inner.latest_snapshot());
        Ok(root_hash)
    }
}

impl Debug for Storage {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Storage").finish_non_exhaustive()
    }
}

/// Gets a value from the verifiable key-value store.
pub(crate) async fn get<S, K>(state: &S, key: K) -> Result<Option<StoredValue>>
where
    S: cnidarium::StateRead + ?Sized,
    K: AsRef<str>,
{
    let key = key.as_ref();
    state
        .get_raw(key)
        .await
        .with_context(|| format!("failed to get raw value under key `{key}`"))?
        .map(|raw| {
            borsh::de::from_slice(&raw)
                .with_context(|| format!("failed to deserialize value under key `{key}`"))
        })
        .transpose()
}

/// Gets a value from the non-verifiable key-value store.
async fn nonverifiable_get<S, K>(state: &S, key: K) -> Result<Option<StoredValue>>
where
    S: cnidarium::StateRead,
    K: AsRef<[u8]>,
{
    let key = key.as_ref();
    state
        .nonverifiable_get_raw(key)
        .await
        .with_context(|| {
            format!(
                "failed to get nonverifiable raw value under key `{}`",
                display_nonverifiable_key(key)
            )
        })?
        .map(|raw| {
            borsh::de::from_slice(&raw).with_context(|| {
                format!(
                    "failed to deserialize nonverifiable value under key `{}`",
                    display_nonverifiable_key(key),
                )
            })
        })
        .transpose()
}

/// Retrieves all keys (but not values) matching a prefix from the verifiable key-value store.
fn prefix_keys<S, K>(state: &S, prefix: K) -> impl Stream<Item = Result<String>> + Send + 'static
where
    S: cnidarium::StateRead,
    K: AsRef<str>,
{
    state.prefix_keys(prefix.as_ref())
}

/// Puts the given value into the verifiable key-value store under the given key.
pub(crate) fn put<S, K>(state: &mut S, key: K, value: &StoredValue) -> Result<()>
where
    S: cnidarium::StateWrite + ?Sized,
    K: Into<String>,
{
    let key = key.into();
    let raw_value = borsh::to_vec(value)
        .with_context(|| format!("failed to serialize value under key `{key}`"))?;
    state.put_raw(key, raw_value);
    Ok(())
}

/// Deletes the key-value from the verifiable key-value store under the given key.
pub(crate) fn delete<S, K>(state: &mut S, key: K)
where
    S: cnidarium::StateWrite + ?Sized,
    K: Into<String>,
{
    state.delete(key.into());
}

/// Puts the given value into the non-verifiable key-value store under the given key.
fn nonverifiable_put<S, K>(state: &mut S, key: K, value: &StoredValue) -> Result<()>
where
    S: cnidarium::StateWrite,
    K: Into<Vec<u8>>,
{
    let key = key.into();
    let raw_value = borsh::to_vec(value).with_context(|| {
        format!(
            "failed to serialize value under key `{}`",
            String::from_utf8(key.clone())
                .unwrap_or_else(|_| telemetry::display::base64(key.as_slice()).to_string())
        )
    })?;
    state.nonverifiable_put_raw(key, raw_value);
    Ok(())
}

/// Deletes the key-value from the non-verifiable key-value store under the given key.
fn nonverifiable_delete<S, K>(state: &mut S, key: K)
where
    S: cnidarium::StateWrite,
    K: Into<Vec<u8>>,
{
    state.nonverifiable_delete(key.into());
}

/// Provides a `String` version of the given key for display (logging) purposes, parsed from UTF-8
/// if possible, falling back to base64 encoding.
pub(crate) fn display_nonverifiable_key(key: &[u8]) -> String {
    String::from_utf8(key.to_vec()).unwrap_or_else(|_| telemetry::display::base64(key).to_string())
}
