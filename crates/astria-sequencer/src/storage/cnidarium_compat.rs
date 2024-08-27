use std::{
    any::Any,
    collections::{
        BTreeMap,
        HashMap,
    },
    sync::Arc,
};

use anyhow::Result;
use astria_core::{
    primitive::v1::{
        asset,
        RollupId,
    },
    sequencerblock::v1alpha1::block::Deposit,
};
use async_trait::async_trait;
use tendermint::abci::Event;

use crate::storage::Storable;

pub(crate) const DELTA_DELTA_KEY: &str = "delta_delta";
type DeltaDeltaInner = cnidarium::StateDelta<Arc<cnidarium::StateDelta<cnidarium::Snapshot>>>;

#[derive(Clone)]
pub(crate) struct SnapshotDeltaCompat {
    inner: Arc<cnidarium::StateDelta<cnidarium::Snapshot>>,
}

impl SnapshotDeltaCompat {
    pub(crate) fn new(snapshot: cnidarium::Snapshot) -> Self {
        Self {
            inner: Arc::new(cnidarium::StateDelta::new(snapshot)),
        }
    }

    pub(crate) fn inner_mut(&mut self) -> Option<&mut cnidarium::StateDelta<cnidarium::Snapshot>> {
        Arc::get_mut(&mut self.inner)
    }

    pub(crate) fn take_inner(self) -> Option<cnidarium::StateDelta<cnidarium::Snapshot>> {
        Arc::try_unwrap(self.inner).ok()
    }
}

pub(crate) struct DeltaDeltaCompat {
    cnidarium: DeltaDeltaInner,
}

impl DeltaDeltaCompat {
    pub(crate) fn new(astria: super::DeltaDelta, cnidarium: SnapshotDeltaCompat) -> Self {
        use cnidarium::StateWrite as _;
        let mut cnidarium = cnidarium::StateDelta::new(cnidarium.inner);
        cnidarium.object_put(DELTA_DELTA_KEY, astria);
        Self {
            cnidarium,
        }
    }

    pub(crate) fn flatten(self) -> cnidarium::Cache {
        self.cnidarium.flatten().1
    }
}

impl cnidarium::StateRead for DeltaDeltaCompat {
    type GetRawFut = <DeltaDeltaInner as cnidarium::StateRead>::GetRawFut;
    type NonconsensusPrefixRawStream =
        <DeltaDeltaInner as cnidarium::StateRead>::NonconsensusPrefixRawStream;
    type NonconsensusRangeRawStream =
        <DeltaDeltaInner as cnidarium::StateRead>::NonconsensusRangeRawStream;
    type PrefixKeysStream = <DeltaDeltaInner as cnidarium::StateRead>::PrefixKeysStream;
    type PrefixRawStream = <DeltaDeltaInner as cnidarium::StateRead>::PrefixRawStream;

    fn get_raw(&self, key: &str) -> Self::GetRawFut {
        self.cnidarium.get_raw(key)
    }

    fn nonverifiable_get_raw(&self, key: &[u8]) -> Self::GetRawFut {
        self.cnidarium.nonverifiable_get_raw(key)
    }

    fn object_get<T: Any + Send + Sync + Clone>(&self, key: &'static str) -> Option<T> {
        self.cnidarium.object_get(key)
    }

    fn object_type(&self, key: &'static str) -> Option<std::any::TypeId> {
        self.cnidarium.object_type(key)
    }

    fn prefix_raw(&self, prefix: &str) -> Self::PrefixRawStream {
        self.cnidarium.prefix_raw(prefix)
    }

    fn prefix_keys(&self, prefix: &str) -> Self::PrefixKeysStream {
        self.cnidarium.prefix_keys(prefix)
    }

    fn nonverifiable_prefix_raw(&self, prefix: &[u8]) -> Self::NonconsensusPrefixRawStream {
        self.cnidarium.nonverifiable_prefix_raw(prefix)
    }

    fn nonverifiable_range_raw(
        &self,
        prefix: Option<&[u8]>,
        range: impl std::ops::RangeBounds<Vec<u8>>,
    ) -> Result<Self::NonconsensusRangeRawStream> {
        self.cnidarium.nonverifiable_range_raw(prefix, range)
    }
}

impl cnidarium::StateWrite for DeltaDeltaCompat {
    fn put_raw(&mut self, key: String, value: Vec<u8>) {
        self.cnidarium.put_raw(key, value)
    }

    fn delete(&mut self, key: String) {
        self.cnidarium.delete(key)
    }

    fn nonverifiable_put_raw(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.cnidarium.nonverifiable_put_raw(key, value)
    }

    fn nonverifiable_delete(&mut self, key: Vec<u8>) {
        self.cnidarium.nonverifiable_delete(key)
    }

    fn object_put<T: Clone + Any + Send + Sync>(&mut self, key: &'static str, value: T) {
        self.cnidarium.object_put(key, value)
    }

    fn object_delete(&mut self, key: &'static str) {
        self.cnidarium.object_delete(key)
    }

    fn object_merge(
        &mut self,
        objects: BTreeMap<&'static str, Option<Box<dyn Any + Send + Sync>>>,
    ) {
        self.cnidarium.object_merge(objects)
    }

    fn record(&mut self, event: Event) {
        self.cnidarium.record(event)
    }
}

// #[async_trait]
// impl super::StateRead for DeltaDeltaCompat {
//     async fn get<K, V>(&self, key: K) -> Result<Option<V>>
//     where
//         K: AsRef<str> + Send,
//         V: Storable,
//     {
//         self.astria.get(key).await
//     }
//
//     async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
//     where
//         K: AsRef<[u8]> + Send,
//         V: Storable,
//     {
//         self.astria.nonverifiable_get(key).await
//     }
// }
//
// impl super::StateWrite for DeltaDeltaCompat {
//     fn put<K, V>(&self, key: K, value: V)
//     where
//         K: Into<String>,
//         V: Storable,
//     {
//         self.astria.put(key, value);
//     }
//
//     fn delete<K: Into<String>>(&self, key: K) {
//         self.astria.delete(key);
//     }
//
//     fn nonverifiable_put<K, V>(&self, key: K, value: V)
//     where
//         K: Into<Vec<u8>>,
//         V: Storable,
//     {
//         self.astria.nonverifiable_put(key, value);
//     }
//
//     fn nonverifiable_delete<K: Into<Vec<u8>>>(&self, key: K) {
//         self.astria.nonverifiable_delete(key);
//     }
//
//     fn record(&self, event: Event) {
//         self.astria.record(event);
//     }
//
//     fn block_fees(&self) -> BTreeMap<asset::IbcPrefixed, u128> {
//         self.astria.block_fees()
//     }
//
//     fn increase_block_fees<TAsset>(&self, asset: TAsset, amount: u128) -> Result<()>
//     where
//         TAsset: Into<asset::IbcPrefixed>,
//     {
//         self.astria.increase_block_fees(asset, amount)
//     }
//
//     fn bridge_deposits(&self) -> HashMap<RollupId, Vec<Deposit>> {
//         self.astria.bridge_deposits()
//     }
//
//     fn put_bridge_deposit(&self, deposit: Deposit) {
//         self.astria.put_bridge_deposit(deposit)
//     }
// }
