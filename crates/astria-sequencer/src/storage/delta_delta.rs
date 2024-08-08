use std::sync::Arc;

// use tendermint::abci::Event;
use super::{
    Cache,
    SnapshotDelta,
};

pub(crate) struct DeltaDelta {
    pub(super) inner: cnidarium::StateDelta<Arc<cnidarium::StateDelta<cnidarium::Snapshot>>>,
    cache: Cache,
}

impl DeltaDelta {
    pub(super) async fn new(mut snapshot_delta: Arc<SnapshotDelta>) -> Self {
        Self {
            inner: cnidarium::StateDelta::new(snapshot_delta.inner()),
            cache: Cache::new_delta(snapshot_delta.cache()).await,
        }
    }

    // pub(crate) fn into_cache(self) -> cnidarium::Cache {
    //     self.inner.flatten().1
    // }

    // pub(crate) fn apply(self) -> Vec<Event> {
    //     self.inner.apply().1
    // }

    pub(super) fn inner(
        &self,
    ) -> &cnidarium::StateDelta<Arc<cnidarium::StateDelta<cnidarium::Snapshot>>> {
        &self.inner
    }

    pub(crate) fn cache(&self) -> &Cache {
        &self.cache
    }
}
