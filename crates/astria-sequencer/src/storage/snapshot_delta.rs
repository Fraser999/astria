use std::sync::Arc;

use super::{
    Cache,
    DeltaDelta,
    Snapshot,
};

pub(crate) struct SnapshotDelta {
    inner: Arc<cnidarium::StateDelta<cnidarium::Snapshot>>,
    cache: Cache,
}

impl SnapshotDelta {
    pub(crate) async fn new(snapshot: Snapshot) -> Self {
        Self {
            inner: Arc::new(cnidarium::StateDelta::new(snapshot.inner().clone())),
            cache: Cache::new_delta(snapshot.cache()).await,
        }
    }

    pub(crate) fn try_begin_transaction(&mut self) -> Option<DeltaDelta> {
        Arc::get_mut(&mut self.inner).map(DeltaDelta::new)
    }

    pub(super) fn inner(&self) -> Arc<cnidarium::StateDelta<cnidarium::Snapshot>> {
        self.inner.clone()
    }

    // pub(crate) fn inner_mut(&mut self) -> &mut cnidarium::StateDelta<cnidarium::Snapshot> {
    //     &mut self.inner
    // }

    pub(crate) fn cache(&self) -> &Cache {
        &self.cache
    }
}
