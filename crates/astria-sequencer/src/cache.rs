use std::{
    fmt::{
        self,
        Debug,
        Formatter,
    },
    sync::{
        Arc,
        Mutex,
    },
};

use astria_core::primitive::v1::RollupId;
use quick_cache::unsync::Cache as QuickCache;

#[derive(Clone)]
pub(crate) struct Cache {
    inner: Arc<Mutex<CacheInner>>,
}

impl Cache {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CacheInner {
                snapshot: QuickCache::new(1_000_000),
                delta: None,
                delta_delta: None,
            })),
        }
    }

    pub(crate) fn new_delta(&self) {
        let mut inner = self.inner.lock().unwrap();

        assert!(inner.delta.is_none());
        assert!(inner.delta_delta.is_none());

        inner.delta = Some(QuickCache::new(1_000_000));
    }

    pub(crate) fn new_delta_delta(&self) {
        let mut inner = self.inner.lock().unwrap();

        assert!(inner.delta.is_some());
        assert!(inner.delta_delta.is_none());

        inner.delta_delta = Some(QuickCache::new(1_000_000));
    }

    pub(crate) fn put(&self, key: Vec<u8>, value: Cached) {
        let mut inner = self.inner.lock().unwrap();
        let cache = if let Some(cache) = inner.delta_delta.as_mut() {
            cache
        } else if let Some(cache) = inner.delta.as_mut() {
            cache
        } else {
            &mut inner.snapshot
        };
        cache.insert(key, value);
    }

    pub(crate) fn get(&self, key: &Vec<u8>) -> Option<Cached> {
        let mut inner = self.inner.lock().unwrap();
        let cache = if let Some(cache) = inner.delta_delta.as_mut() {
            cache
        } else if let Some(cache) = inner.delta.as_mut() {
            cache
        } else {
            &mut inner.snapshot
        };
        cache.get_mut(key).and_then(|value| {
            if matches!(*value, Cached::Deleted) {
                None
            } else {
                Some((*value).clone())
            }
        })
    }

    /// Don't actually delete from cache, instead put `Cached::Deleted` entry.
    pub(crate) fn delete(&self, key: Vec<u8>) {
        let mut inner = self.inner.lock().unwrap();
        let cache = if let Some(cache) = inner.delta_delta.as_mut() {
            cache
        } else if let Some(cache) = inner.delta.as_mut() {
            cache
        } else {
            &mut inner.snapshot
        };
        cache.insert(key, Cached::Deleted);
    }

    pub(crate) fn apply_delta(&self) {
        let mut inner = self.inner.lock().unwrap();

        assert!(inner.delta.is_some());
        assert!(inner.delta_delta.is_none());

        inner
            .delta
            .take()
            .unwrap()
            .drain()
            .for_each(|(key, value)| inner.snapshot.insert(key, value));
    }

    pub(crate) fn apply_delta_delta(&self) {
        let mut inner = self.inner.lock().unwrap();

        assert!(inner.delta.is_some());
        assert!(inner.delta_delta.is_some());

        let mut cache = inner.delta_delta.take().unwrap();
        let delta = inner.delta.as_mut().expect("cannot be None");
        cache
            .drain()
            .for_each(|(key, value)| delta.insert(key, value));
    }

    pub(crate) fn discard_changes(&self) {
        let mut inner = self.inner.lock().unwrap();

        if let Some(mut cache) = inner.delta_delta.take() {
            assert!(inner.delta.is_some());
            cache.clear();
            return;
        }

        if let Some(mut cache) = inner.delta.take() {
            cache.clear();
            return;
        }

        tracing::error!("can't apply changes to snapshot cache");
    }
}

impl Debug for Cache {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Cache").finish_non_exhaustive()
    }
}

struct CacheInner {
    snapshot: QuickCache<Vec<u8>, Cached>,
    delta: Option<QuickCache<Vec<u8>, Cached>>,
    delta_delta: Option<QuickCache<Vec<u8>, Cached>>,
}

#[derive(Clone, Debug)]
pub(crate) enum Cached {
    Deleted,
    Balance(u128),
    Nonce(u32),
    TransferBaseFee(u128),
    BasePrefix(String),
    IsAllowedFeeAsset(bool),
    BlockFee(u128),
    RollupId(RollupId),
}
