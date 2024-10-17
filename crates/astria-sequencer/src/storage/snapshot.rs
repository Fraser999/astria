use std::{
    any::{
        Any,
        TypeId,
    },
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{
        Context,
        Poll,
    },
};

use astria_eyre::anyhow_to_eyre;
use async_trait::async_trait;
use cnidarium::{
    RootHash,
    StateDelta,
    StateRead,
};
use futures::{
    Stream,
    TryFutureExt,
};
use pin_project_lite::pin_project;
use quick_cache::sync::Cache as QuickCache;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    CachedValue,
    StoredValue,
};

#[derive(Clone)]
pub(crate) struct Snapshot {
    /// The underlying snapshot of the DB.
    inner: cnidarium::Snapshot,
    /// An in-memory cache of objects that belong in the verifiable store.
    verifiable_cache: Arc<QuickCache<String, CachedValue>>,
    /// An in-memory cache of objects that belong in the non-verifiable store.
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

    // pub(super) fn inner(&self) -> cnidarium::Snapshot {
    //     self.inner.clone()
    // }

    pub(crate) fn new_delta(&self) -> StateDelta<Snapshot> {
        StateDelta::new(self.clone())
    }

    // pub(crate) fn new_cnidarium_delta(&self) -> SnapshotDeltaCompat {
    //     SnapshotDeltaCompat::new(self.inner())
    // }

    pub(crate) async fn root_hash(&self) -> astria_eyre::Result<RootHash> {
        self.inner.root_hash().await.map_err(anyhow_to_eyre)
    }

    // pub(crate) fn prefix_keys<K>(
    //     &self,
    //     prefix: K,
    // ) -> impl Stream<Item = Result<String>> + Send + 'static
    // where
    //     K: AsRef<str>,
    // {
    //     super::prefix_keys(&self.inner, prefix)
    // }
}

#[async_trait]
impl StateRead for Snapshot {
    type GetRawFut = SnapshotFuture;
    type NonconsensusPrefixRawStream = ReceiverStream<anyhow::Result<(Vec<u8>, Vec<u8>)>>;
    type NonconsensusRangeRawStream = ReceiverStream<anyhow::Result<(Vec<u8>, Vec<u8>)>>;
    type PrefixKeysStream = ReceiverStream<anyhow::Result<String>>;
    type PrefixRawStream = ReceiverStream<anyhow::Result<(String, Vec<u8>)>>;

    fn get_raw(&self, key: &str) -> Self::GetRawFut {
        SnapshotFuture::new(|| async {
            self.verifiable_cache
                .get_or_insert_async(key, super::get(&self.inner, key).ok_into::<CachedValue>())
                .ok_into::<Option<StoredValue<'static>>>()
                .await?
                .map(|value| {
                    borsh::to_vec(value).expect("should serialize as previously succeeded")
                })
                .transpose()
        })
    }

    fn nonverifiable_get_raw(&self, key: &[u8]) -> Self::GetRawFut {
        SnapshotFuture::new(|| async {
            self.nonverifiable_cache
                .get_or_insert_async(key, super::get(&self.inner, key).ok_into::<CachedValue>())
                .ok_into::<Option<StoredValue<'static>>>()
                .await?
                .map(|value| {
                    borsh::to_vec(value).expect("should serialize as previously succeeded")
                })
                .transpose()
        })
    }

    fn object_get<T: Any + Send + Sync + Clone>(&self, _key: &str) -> Option<T> {
        // No ephemeral object cache in read-only `Snapshot`.
        None
    }

    fn object_type(&self, _key: &str) -> Option<TypeId> {
        // No ephemeral object cache in read-only `Snapshot`.
        None
    }

    fn prefix_raw(&self, prefix: &str) -> Self::PrefixRawStream {
        todo!();
    }

    fn prefix_keys(&self, prefix: &str) -> Self::PrefixKeysStream {
        todo!();
    }

    fn nonverifiable_prefix_raw(&self, prefix: &[u8]) -> Self::NonconsensusPrefixRawStream {
        todo!();
    }

    fn nonverifiable_range_raw(
        &self,
        prefix: Option<&[u8]>,
        range: impl std::ops::RangeBounds<Vec<u8>>,
    ) -> anyhow::Result<Self::NonconsensusRangeRawStream> {
    }
}

pin_project! {
    pub struct SnapshotFuture {
        #[pin]
        inner: tokio::task::JoinHandle<anyhow::Result<Option<Vec<u8>>>>
    }
}

impl SnapshotFuture {
    fn new<F>(future: F) -> Self
    where
        F: Future<Output = anyhow::Result<Option<Vec<u8>>>> + Send + 'static,
    {
        Self {
            inner: tokio::task::spawn(future),
        }
    }
}

impl Future for SnapshotFuture {
    type Output = anyhow::Result<Option<Vec<u8>>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.inner.poll(cx) {
            Poll::Ready(result) => {
                Poll::Ready(result.expect("unrecoverable join error from tokio task"))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
