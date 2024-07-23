mod delta_delta;
mod snapshot;
mod snapshot_delta;
mod state;

use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};
use cnidarium::{
    RootHash,
    StagedWriteBatch,
};
use futures::{
    future,
    Stream,
    TryStreamExt,
};

pub(crate) use self::{
    delta_delta::DeltaDelta,
    snapshot::Snapshot,
    snapshot_delta::SnapshotDelta,
    state::{
        StateRead,
        StateWrite,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct Storage {
    inner: cnidarium::Storage,
    #[cfg(test)]
    _temp_dir: Option<std::sync::Arc<tempfile::TempDir>>,
}

impl Storage {
    pub(crate) async fn init(path: PathBuf, prefixes: Vec<String>) -> Result<Self> {
        let inner = cnidarium::Storage::init(path, prefixes).await?;
        Ok(Self {
            inner,
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
        Snapshot::new(self.inner.latest_snapshot())
    }

    /// Returns the [`Snapshot`] corresponding to the given version.
    pub fn snapshot(&self, version: u64) -> Option<Snapshot> {
        Some(Snapshot::new(self.inner.snapshot(version)?))
    }

    /// Returns a new [`SnapshotDelta`] on top of the latest version of the tree.
    pub fn latest_snapshot_delta(&self) -> SnapshotDelta {
        SnapshotDelta::new(self.latest_snapshot())
    }

    /// Prepares a commit for the provided [`SnapshotDelta`], returning a [`StagedWriteBatch`].
    ///
    /// The batch can be committed to the database using the [`Storage::commit_batch`] method.
    pub async fn prepare_commit(&self, delta: SnapshotDelta) -> Result<StagedWriteBatch> {
        self.inner
            .prepare_commit(std::sync::Arc::into_inner(delta.inner()).unwrap())
            .await
    }

    /// Commits the provided [`StateDelta`] to persistent storage as the latest version of the chain
    /// state.
    pub async fn commit(&self, delta: SnapshotDelta) -> Result<RootHash> {
        self.inner
            .commit(std::sync::Arc::into_inner(delta.inner()).unwrap())
            .await
    }

    /// Commits the supplied [`StagedWriteBatch`] to persistent storage.
    pub fn commit_batch(&self, batch: StagedWriteBatch) -> Result<RootHash> {
        self.inner.commit_batch(batch)
    }
}

/// Gets a value from the verifiable key-value store.
async fn get<S, K, V>(state: &S, key: K) -> Result<Option<V>>
where
    S: cnidarium::StateRead,
    K: AsRef<str>,
    V: BorshDeserialize,
{
    let key = key.as_ref();
    state
        .get_raw(key)
        .await
        .with_context(|| format!("failed to get raw value under key `{key}`"))?
        .map(|raw| {
            borsh::de::from_slice(&raw).with_context(|| {
                format!(
                    "failed to deserialize value under key `{key}` to `{}`",
                    std::any::type_name::<V>()
                )
            })
        })
        .transpose()
}

/// Gets a value from the non-verifiable key-value store.
async fn nonverifiable_get<S, K, V>(state: &S, key: K) -> Result<Option<V>>
where
    S: cnidarium::StateRead,
    K: AsRef<[u8]>,
    V: BorshDeserialize,
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
                    "failed to deserialize nonverifiable value under key `{}` to `{}`",
                    display_nonverifiable_key(key),
                    std::any::type_name::<V>()
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

/// Retrieves all key-value pairs for keys matching a prefix from the non-verifiable key-value
/// store.
fn nonverifiable_prefix<S, K, V>(
    state: &S,
    prefix: K,
) -> impl Stream<Item = Result<(Vec<u8>, V)>> + Send + 'static
where
    S: cnidarium::StateRead,
    K: AsRef<[u8]>,
    V: BorshDeserialize + Send + 'static,
{
    let prefix = prefix.as_ref();

    state
        .nonverifiable_prefix_raw(prefix)
        .and_then(|(raw_key, raw_value)| {
            match borsh::de::from_slice(&raw_value).with_context(|| {
                format!(
                    "failed to deserialize nonverifiable value under key `{}` to `{}`",
                    display_nonverifiable_key(&raw_key),
                    std::any::type_name::<V>()
                )
            }) {
                Ok(value) => future::ok((raw_key, value)),
                Err(error) => future::err(error),
            }
        })
}

/// Puts the given value into the verifiable key-value store under the given key.
fn put<S, K, V>(state: &mut S, key: K, value: &V) -> Result<()>
where
    S: cnidarium::StateWrite,
    K: Into<String>,
    V: BorshSerialize,
{
    let key = key.into();
    let raw_value = borsh::to_vec(value)
        .with_context(|| format!("failed to serialize value under key `{key}`"))?;
    Ok(state.put_raw(key, raw_value))
}

/// Deletes the key-value from the verifiable key-value store under the given key.
fn delete<S, K>(state: &mut S, key: K)
where
    S: cnidarium::StateWrite,
    K: Into<String>,
{
    state.delete(key.into())
}

/// Puts the given value into the non-verifiable key-value store under the given key.
fn nonverifiable_put<S, K, V>(state: &mut S, key: K, value: &V) -> Result<()>
where
    S: cnidarium::StateWrite,
    K: Into<Vec<u8>>,
    V: BorshSerialize,
{
    let key = key.into();
    let raw_value = borsh::to_vec(value).with_context(|| {
        format!(
            "failed to serialize value under key `{}`",
            String::from_utf8(key.clone())
                .unwrap_or_else(|_| telemetry::display::base64(key.as_slice()).to_string())
        )
    })?;
    Ok(state.nonverifiable_put_raw(key, raw_value))
}

/// Deletes the key-value from the non-verifiable key-value store under the given key.
fn nonverifiable_delete<S, K>(state: &mut S, key: K)
where
    S: cnidarium::StateWrite,
    K: Into<Vec<u8>>,
{
    state.nonverifiable_delete(key.into())
}

/// Provides a `String` version of the given key for display (logging) purposes, parsed from UTF-8
/// if possible, falling back to base64 encoding.
pub(crate) fn display_nonverifiable_key(key: &[u8]) -> String {
    String::from_utf8(key.to_vec()).unwrap_or_else(|_| telemetry::display::base64(key).to_string())
}
