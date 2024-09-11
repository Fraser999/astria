use std::{
    any::{
        Any,
        TypeId,
    },
    future::Future,
    ops::RangeBounds,
    pin::Pin,
    sync::Arc,
    task::{
        Context,
        Poll,
    },
};

use anyhow::Result;
use futures::TryFutureExt;
use pin_project_lite::pin_project;
use quick_cache::sync::Cache as QuickCache;

#[derive(Clone, Debug)]
pub(super) enum CachedValue {
    /// Was either not in the on-disk storage, or is due to be deleted from there.
    Absent,
    /// Is either present in the on-disk storage, or is due to be added there.
    Stored(Vec<u8>),
}

impl From<Option<Vec<u8>>> for CachedValue {
    fn from(value: Option<Vec<u8>>) -> Self {
        match value {
            None => CachedValue::Absent,
            Some(stored) => CachedValue::Stored(stored),
        }
    }
}

impl From<CachedValue> for Option<Vec<u8>> {
    fn from(value: CachedValue) -> Self {
        match value {
            CachedValue::Absent => None,
            CachedValue::Stored(stored) => Some(stored),
        }
    }
}
// #[derive(Clone, Debug)]
// pub(super) enum CachedValue {
//     /// Was either not in the on-disk storage, or is due to be deleted from there.
//     Absent,
//     /// Is either present in the on-disk storage, or is due to be added there.
//     Stored(Arc<dyn Any + Send + Sync>),
// }
//
// impl From<Option<Arc<dyn Any + Send + Sync>>> for CachedValue {
//     fn from(value: Option<Arc<dyn Any + Send + Sync>>) -> Self {
//         match value {
//             None => CachedValue::Absent,
//             Some(stored) => CachedValue::Stored(stored),
//         }
//     }
// }
//
// impl From<CachedValue> for Option<Arc<dyn Any + Send + Sync>> {
//     fn from(value: CachedValue) -> Self {
//         match value {
//             CachedValue::Absent => None,
//             CachedValue::Stored(stored) => Some(stored),
//         }
//     }
// }

#[derive(Clone)]
pub(crate) struct Snapshot {
    inner: cnidarium::Snapshot,
    verifiable_cache: Arc<QuickCache<String, CachedValue>>,
    nonverifiable_cache: Arc<QuickCache<Vec<u8>, CachedValue>>,
}

pin_project! {
    /// Future representing a read from a state snapshot.
    pub(crate) struct SnapshotFuture {
        #[pin]
        inner: tokio::task::JoinHandle<Result<Option<Vec<u8>>>>
    }
}

impl Future for SnapshotFuture {
    type Output = Result<Option<Vec<u8>>>;

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

impl cnidarium::StateRead for Snapshot {
    type GetRawFut = SnapshotFuture;
    type NonconsensusPrefixRawStream =
        <cnidarium::Snapshot as cnidarium::StateRead>::NonconsensusPrefixRawStream;
    type NonconsensusRangeRawStream =
        <cnidarium::Snapshot as cnidarium::StateRead>::NonconsensusRangeRawStream;
    type PrefixKeysStream = <cnidarium::Snapshot as cnidarium::StateRead>::PrefixKeysStream;
    type PrefixRawStream = <cnidarium::Snapshot as cnidarium::StateRead>::PrefixRawStream;

    fn get_raw(&self, key: &str) -> Self::GetRawFut {
        let key = key.to_owned();
        let inner = self.inner.clone();
        let cache = self.verifiable_cache.clone();
        SnapshotFuture {
            inner: tokio::spawn(async move {
                cache
                    .get_or_insert_async(&key, inner.get_raw(&key).ok_into::<CachedValue>())
                    .await
                    .map(Option::<Vec<u8>>::from)
            }),
        }
    }

    fn nonverifiable_get_raw(&self, key: &[u8]) -> Self::GetRawFut {
        let key = key.to_owned();
        let inner = self.inner.clone();
        let cache = self.nonverifiable_cache.clone();
        SnapshotFuture {
            inner: tokio::spawn(async move {
                cache
                    .get_or_insert_async(
                        &key,
                        inner
                            .clone()
                            .nonverifiable_get_raw(&key)
                            .ok_into::<CachedValue>(),
                    )
                    .await
                    .map(Option::<Vec<u8>>::from)
            }),
        }
    }

    fn object_get<T: Any + Send + Sync + Clone>(&self, key: &'static str) -> Option<T> {
        self.inner.object_get(key)
    }

    fn object_type(&self, key: &'static str) -> Option<TypeId> {
        self.inner.object_type(key)
    }

    fn prefix_raw(&self, prefix: &str) -> Self::PrefixRawStream {
        todo!()
    }

    fn prefix_keys(&self, prefix: &str) -> Self::PrefixKeysStream {
        todo!()
    }

    fn nonverifiable_prefix_raw(&self, prefix: &[u8]) -> Self::NonconsensusPrefixRawStream {
        todo!()
    }

    fn nonverifiable_range_raw(
        &self,
        prefix: Option<&[u8]>,
        range: impl RangeBounds<Vec<u8>>,
    ) -> anyhow::Result<Self::NonconsensusRangeRawStream> {
        todo!()
    }
}
