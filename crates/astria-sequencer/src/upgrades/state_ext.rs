use astria_core::upgrades::v1::{
    Change,
    ChangeHash,
};
use astria_eyre::{
    anyhow_to_eyre,
    eyre::{
        bail,
        Result,
        WrapErr as _,
    },
};
use async_trait::async_trait;
use cnidarium::{
    StateRead,
    StateWrite,
};
use tracing::instrument;

use super::{
    storage::{
        self,
        keys,
    },
    Upgrade,
};
use crate::{
    accounts::AddressBytes,
    storage::StoredValue,
};

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_upgrade_change_hash(
        &self,
        upgrade: &dyn Upgrade,
        change: &dyn Change,
    ) -> Result<Option<ChangeHash>> {
        let Some(bytes) = self
            .get_raw(&keys::change(upgrade.name(), change.name()))
            .await
            .map_err(anyhow_to_eyre)
            .wrap_err("failed reading change hash from state")?
        else {
            return Ok(None);
        };
        StoredValue::deserialize(&bytes)
            .and_then(|value| {
                storage::ChangeHash::try_from(value).map(|hash| Some(ChangeHash::from(hash)))
            })
            .wrap_err("invalid change hash bytes")
    }
}

impl<T: StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_upgrade_change_hash(&mut self, upgrade: &dyn Upgrade, change: &dyn Change) -> Result<()> {
        let change_hash = change.calculate_hash();
        let bytes = StoredValue::from(storage::ChangeHash::from(&change_hash))
            .serialize()
            .wrap_err("failed to serialize change hash")?;
        self.put_raw(keys::change(upgrade.name(), change.name()), bytes);
        Ok(())
    }
}

impl<T: StateWrite> StateWriteExt for T {}

#[cfg(test)]
mod tests {
    use cnidarium::StateDelta;

    use super::*;

    struct TestUpgrade;

    impl Upgrade for TestUpgrade {
        fn activation_height(&self) -> u64 {
            10
        }

        fn shutdown_required(&self) -> bool {
            false
        }

        fn name(&self) -> &'static str {
            "test_upgrade"
        }
    }

    #[derive(borsh::BorshSerialize)]
    struct TestChange;

    impl Change for TestChange {
        fn activation_height(&self) -> u64 {
            10
        }

        fn app_version(&self) -> u64 {
            2
        }

        fn name(&self) -> &'static str {
            "test_change"
        }
    }

    #[tokio::test]
    async fn change_hash_roundtrip() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        assert!(
            state
                .get_upgrade_change_hash(&TestUpgrade, &TestChange)
                .await
                .unwrap()
                .is_none()
        );

        state.put_upgrade_change_hash(&TestUpgrade, &TestChange).unwrap();

        assert_eq!(
            state
                .get_upgrade_change_hash(&TestUpgrade, &TestChange)
                .await
                .unwrap(),
            Some(TestChange.calculate_hash()),
        );
    }
}
