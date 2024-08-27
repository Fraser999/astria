use std::{
    collections::{
        BTreeMap,
        HashMap,
    },
    sync::{
        Arc,
        Mutex,
    },
};

use anyhow::{
    Context,
    Result,
};
use astria_core::{
    primitive::v1::{
        asset,
        RollupId,
    },
    sequencerblock::v1alpha1::block::Deposit,
};
use async_trait::async_trait;
use tendermint::abci;
use tracing::error;

use super::{
    CachedValue,
    Snapshot,
    StateRead,
    StateWrite,
    Storable,
};

pub(crate) type SnapshotDelta = Delta<Snapshot>;
pub(crate) type DeltaDelta = Delta<SnapshotDelta>;

#[derive(Default)]
pub(super) struct DeltaInner {
    /// Changes pending on the verifiable store.  `CachedValue::Absent` represents a pending
    /// deletion.
    pub(super) verifiable_changes: HashMap<String, CachedValue>,
    /// Changes pending on the non-verifiable store.  `CachedValue::Absent` represents a pending
    /// deletion.
    pub(super) nonverifiable_changes: HashMap<Vec<u8>, CachedValue>,
    /// The collection of block fees relevant to the current delta.
    pub(super) block_fees: BTreeMap<asset::IbcPrefixed, u128>,
    /// The collection of bridge deposits relevant to the current delta.
    pub(super) bridge_deposits: HashMap<RollupId, Vec<Deposit>>,
    /// The collection of ABCI events relevant to the current delta.
    pub(super) events: Vec<abci::Event>,
}

#[derive(Clone)]
pub(crate) struct Delta<T> {
    parent: T,
    delta: Arc<Mutex<Option<DeltaInner>>>,
}

impl<T> Delta<T> {
    pub(super) fn new(parent: T) -> Self {
        Self {
            parent,
            delta: Arc::new(Mutex::new(Some(DeltaInner::default()))),
        }
    }
}

impl SnapshotDelta {
    pub(crate) fn new_delta(&self) -> DeltaDelta {
        DeltaDelta {
            parent: self.clone(),
            delta: Arc::new(Mutex::new(Some(DeltaInner {
                verifiable_changes: HashMap::default(),
                nonverifiable_changes: HashMap::default(),
                // NOTE: cloned from parent.  If applied later, will overwrite parent values.
                block_fees: self.block_fees(),
                // NOTE: cloned from parent.  If applied later, will overwrite parent values.
                bridge_deposits: self.bridge_deposits(),
                events: vec![],
            }))),
        }
    }

    pub(super) fn consume(self) -> Option<DeltaInner> {
        self.delta.lock().unwrap().take()
    }
}

impl DeltaDelta {
    pub(crate) fn apply(self) -> Vec<abci::Event> {
        if let Some(child_delta) = self.delta.lock().unwrap().take() {
            if let Some(parent_delta) = self.parent.delta.lock().unwrap().as_mut() {
                parent_delta
                    .verifiable_changes
                    .extend(child_delta.verifiable_changes);
                parent_delta
                    .nonverifiable_changes
                    .extend(child_delta.nonverifiable_changes);
                parent_delta.block_fees.extend(child_delta.block_fees);
                parent_delta
                    .bridge_deposits
                    .extend(child_delta.bridge_deposits);
                child_delta.events
            } else {
                error!("parent delta is already applied");
                panic!();
            }
        } else {
            error!("child delta is already applied");
            panic!();
        }
    }
}

#[async_trait]
impl<T: StateRead> StateRead for Delta<T> {
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str> + Send,
        V: Storable,
    {
        if let Some(delta) = self.delta.lock().unwrap().as_ref() {
            match delta.verifiable_changes.get(key.as_ref()) {
                Some(CachedValue::Absent) => return Ok(None),
                Some(CachedValue::Stored(stored)) => {
                    let value = V::try_from(stored.clone())?;
                    return Ok(Some(value));
                }
                None => {}
            }
        } else {
            error!("delta is already applied");
            panic!();
        }
        self.parent.get(key).await
    }

    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + Send,
        V: Storable,
    {
        if let Some(delta) = self.delta.lock().unwrap().as_ref() {
            match delta.nonverifiable_changes.get(key.as_ref()) {
                Some(CachedValue::Absent) => return Ok(None),
                Some(CachedValue::Stored(stored)) => {
                    let value = V::try_from(stored.clone())?;
                    return Ok(Some(value));
                }
                None => {}
            }
        } else {
            error!("delta is already applied");
            panic!();
        }
        self.parent.nonverifiable_get(key).await
    }
}

impl<T: StateRead> StateWrite for Delta<T> {
    fn put<K, V>(&self, key: K, value: V)
    where
        K: Into<String>,
        V: Storable,
    {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta
                .verifiable_changes
                .insert(key.into(), CachedValue::Stored(value.into()));
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn delete<K: Into<String>>(&self, key: K) {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta
                .verifiable_changes
                .insert(key.into(), CachedValue::Absent);
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn nonverifiable_put<K, V>(&self, key: K, value: V)
    where
        K: Into<Vec<u8>>,
        V: Storable,
    {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta
                .nonverifiable_changes
                .insert(key.into(), CachedValue::Stored(value.into()));
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn nonverifiable_delete<K: Into<Vec<u8>>>(&self, key: K) {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta
                .nonverifiable_changes
                .insert(key.into(), CachedValue::Absent);
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn record(&self, event: abci::Event) {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta.events.push(event);
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn block_fees(&self) -> BTreeMap<asset::IbcPrefixed, u128> {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta.block_fees.clone()
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn increase_block_fees<TAsset>(&self, asset: TAsset, amount: u128) -> Result<()>
    where
        TAsset: Into<asset::IbcPrefixed>,
    {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            let fee = delta.block_fees.entry(asset.into()).or_insert(0);
            *fee = (*fee)
                .checked_add(amount)
                .context("block fees overflowed u128")?;
        } else {
            error!("delta is already applied");
            panic!();
        }
        Ok(())
    }

    fn bridge_deposits(&self) -> HashMap<RollupId, Vec<Deposit>> {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta.bridge_deposits.clone()
        } else {
            error!("delta is already applied");
            panic!();
        }
    }

    fn put_bridge_deposit(&self, deposit: Deposit) {
        if let Some(delta) = self.delta.lock().unwrap().as_mut() {
            delta
                .bridge_deposits
                .entry(*deposit.rollup_id())
                .or_default()
                .push(deposit);
        } else {
            error!("delta is already applied");
            panic!();
        }
    }
}
