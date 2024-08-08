use std::{
    fmt::{
        self,
        Debug,
        Formatter,
    },
    rc::Rc,
    sync::Arc,
};

use quick_cache::{
    sync::DefaultLifecycle,
    unsync::{
        Cache as QuickCache,
        RefMut,
    },
    DefaultHashBuilder,
    UnitWeighter,
};
use tokio::sync::RwLock;

use super::Cached;

#[derive(Clone)]
pub(crate) struct Cache {
    inner: Arc<RwLock<CacheInner>>,
}

impl Cache {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(CacheInner {
                snapshot: Rc::new(QuickCache::new(1_000_000)),
                delta: None,
                delta_delta: None,
            })),
        }
    }

    pub(crate) async fn new_delta(cache: &Cache) -> Self {
        let inner = cache.inner.read().await;

        if inner.delta.is_none() {
            return Self {
                inner: Arc::new(RwLock::new(CacheInner {
                    snapshot: inner.snapshot.clone(),
                    delta: Some(Rc::new(QuickCache::new(1_000_000))),
                    delta_delta: None,
                })),
            };
        }

        if inner.delta_delta.is_none() {
            return Self {
                inner: Arc::new(RwLock::new(CacheInner {
                    snapshot: inner.snapshot.clone(),
                    delta: inner.delta.clone(),
                    delta_delta: Some(QuickCache::new(1_000_000)),
                })),
            };
        }

        panic!("can't create new delta from a delta-delta")
    }

    pub(crate) async fn put(&self, key: Vec<u8>, value: Cached) {
        let mut inner = self.inner.write().await;
        let cache = if let Some(cache) = inner.delta_delta.as_mut() {
            cache
        } else if let Some(cache) = inner.delta.as_mut() {
            &mut *cache
        } else {
            &mut *inner.snapshot
        };
        cache.insert(key, value);
    }

    pub(crate) async fn get(
        &self,
        key: &Vec<u8>,
    ) -> Option<
        RefMut<
            '_,
            Vec<u8>,
            Cached,
            UnitWeighter,
            DefaultHashBuilder,
            DefaultLifecycle<Vec<u8>, Cached>,
        >,
    > {
        let inner = self.inner.write().await;
        let cache = if let Some(cache) = inner.delta_delta.as_mut() {
            cache
        } else if let Some(cache) = inner.delta.as_mut() {
            &mut *cache
        } else {
            &mut *inner.snapshot
        };
        cache.get_mut(key).and_then(|value| {
            if matches!(*value, Cached::Deleted) {
                None
            } else {
                Some(value)
            }
        })
    }

    /// Don't actually delete from cache, instead put `Cached::Deleted` entry.
    pub(crate) async fn delete(&self, key: Vec<u8>) {
        let inner = self.inner.write().await;
        let cache = if let Some(cache) = inner.delta_delta.as_mut() {
            cache
        } else if let Some(cache) = inner.delta.as_mut() {
            &mut *cache
        } else {
            &mut *inner.snapshot
        };
        cache.insert(key, Cached::Deleted);
    }

    pub(crate) async fn apply_changes(self) -> Self {
        let inner = self.inner.write().await;

        if let Some(mut cache) = inner.delta_delta.take() {
            let delta = inner.delta.as_mut().expect("cannot be None");
            cache
                .drain()
                .for_each(|(key, value)| delta.insert(key, value));
            return self;
        }

        if let Some(mut cache) = inner.delta.take() {
            cache
                .drain()
                .for_each(|(key, value)| inner.snapshot.insert(key, value));
            return self;
        }

        tracing::error!("can't apply changes to snapshot cache");
        self
    }
}

impl Debug for Cache {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Cache").finish_non_exhaustive()
    }
}

struct CacheInner {
    snapshot: Rc<QuickCache<Vec<u8>, Cached>>,
    delta: Option<Rc<QuickCache<Vec<u8>, Cached>>>,
    delta_delta: Option<QuickCache<Vec<u8>, Cached>>,
}
