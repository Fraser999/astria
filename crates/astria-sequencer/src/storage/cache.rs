// use std::{
//     fmt::{
//         self,
//         Debug,
//         Formatter,
//     },
//     sync::{
//         Arc,
//         Mutex,
//         Weak,
//     },
// };
//
// use astria_core::primitive::v1::RollupId;
// use quick_cache::sync::Cache as QuickCache;

use super::StoredValue;

// #[derive(Clone)]
// pub(crate) struct Cache {
// inner: Arc<QuickCache<Vec<u8>, CachedValue>>,
// parent: Option<Cache>,
// }
//
// impl Cache {
// pub(crate) fn new() -> Self {
// Self {
// inner: Arc::new(Mutex::new(CacheInner {
// snapshot: QuickCache::new(1_000_000),
// delta: None,
// delta_delta: None,
// })),
// }
// }
//
// pub(crate) fn new_delta(&self) {
// let mut inner = self.inner.lock().unwrap();
//
// assert!(inner.delta.is_none());
// assert!(inner.delta_delta.is_none());
//
// inner.delta = Some(QuickCache::new(1_000_000));
// }
//
// pub(crate) fn new_delta_delta(&self) {
// let mut inner = self.inner.lock().unwrap();
//
// assert!(inner.delta.is_some());
// assert!(inner.delta_delta.is_none());
//
// inner.delta_delta = Some(QuickCache::new(1_000_000));
// }
//
// pub(crate) fn put(&self, key: Vec<u8>, value: CachedValue) {
// let mut inner = self.inner.lock().unwrap();
// let cache = if let Some(cache) = inner.delta_delta.as_mut() {
// cache
// } else if let Some(cache) = inner.delta.as_mut() {
// cache
// } else {
// &mut inner.snapshot
// };
// cache.insert(key, value);
// }
//
// pub(crate) fn get(&self, key: &Vec<u8>) -> Option<CachedValue> {
// let mut inner = self.inner.lock().unwrap();
// let cache = if let Some(cache) = inner.delta_delta.as_mut() {
// cache
// } else if let Some(cache) = inner.delta.as_mut() {
// cache
// } else {
// &mut inner.snapshot
// };
// cache.get_mut(key).and_then(|value| {
// if matches!(*value, CachedValue::Deleted) {
// None
// } else {
// Some((*value).clone())
// }
// })
// }
//
// Don't actually delete from cache, instead put `Cached::Deleted` entry.
// pub(crate) fn delete(&self, key: Vec<u8>) {
// let mut inner = self.inner.lock().unwrap();
// let cache = if let Some(cache) = inner.delta_delta.as_mut() {
// cache
// } else if let Some(cache) = inner.delta.as_mut() {
// cache
// } else {
// &mut inner.snapshot
// };
// cache.insert(key, CachedValue::Deleted);
// }
//
// pub(crate) fn apply_delta(&self) {
// let mut inner = self.inner.lock().unwrap();
//
// assert!(inner.delta.is_some());
// assert!(inner.delta_delta.is_none());
//
// inner
// .delta
// .take()
// .unwrap()
// .drain()
// .for_each(|(key, value)| inner.snapshot.insert(key, value));
// }
//
// pub(crate) fn apply_delta_delta(&self) {
// let mut inner = self.inner.lock().unwrap();
//
// assert!(inner.delta.is_some());
// assert!(inner.delta_delta.is_some());
//
// let mut cache = inner.delta_delta.take().unwrap();
// let delta = inner.delta.as_mut().expect("cannot be None");
// cache
// .drain()
// .for_each(|(key, value)| delta.insert(key, value));
// }
//
// pub(crate) fn discard_changes(&self) {
// let mut inner = self.inner.lock().unwrap();
//
// if let Some(mut cache) = inner.delta_delta.take() {
// assert!(inner.delta.is_some());
// cache.clear();
// return;
// }
//
// if let Some(mut cache) = inner.delta.take() {
// cache.clear();
// return;
// }
//
// tracing::error!("can't apply changes to snapshot cache");
// }
// }
//
// impl Debug for Cache {
// fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
// formatter.debug_struct("Cache").finish_non_exhaustive()
// }
// }
//
// struct CacheInner {
// snapshot: QuickCache<Vec<u8>, CachedValue>,
// delta: Option<QuickCache<Vec<u8>, CachedValue>>,
// delta_delta: Option<QuickCache<Vec<u8>, CachedValue>>,
// }

#[derive(Clone, Debug)]
pub(super) enum CachedValue {
    /// Was either not in the on-disk storage, or is due to be deleted from there.
    Absent,
    /// Is either present in the on-disk storage, or is due to be added there.
    Stored(StoredValue),
}

impl From<Option<StoredValue>> for CachedValue {
    fn from(value: Option<StoredValue>) -> Self {
        match value {
            None => CachedValue::Absent,
            Some(stored) => CachedValue::Stored(stored),
        }
    }
}

impl From<CachedValue> for Option<StoredValue> {
    fn from(value: CachedValue) -> Self {
        match value {
            CachedValue::Absent => None,
            CachedValue::Stored(stored_value) => Some(stored_value),
        }
    }
}
