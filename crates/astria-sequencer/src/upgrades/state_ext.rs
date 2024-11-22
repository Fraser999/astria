use std::collections::BTreeSet;

use astria_core::upgrades::v1::{
    Change,
    ChangeInfo,
    ChangeName,
    UpgradeName,
};
use astria_eyre::{
    anyhow_to_eyre,
    eyre::{
        Result,
        WrapErr as _,
    },
};
use async_trait::async_trait;
use cnidarium::{
    StateRead,
    StateWrite,
};
use futures::TryStreamExt;
use tracing::instrument;

use super::storage::{
    self,
    keys,
};
use crate::storage::StoredValue;

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_upgrade_change_info(
        &self,
        upgrade_name: &UpgradeName,
        change_name: &ChangeName,
    ) -> Result<Option<ChangeInfo>> {
        let Some(bytes) = self
            .get_raw(&keys::change(upgrade_name, change_name))
            .await
            .map_err(anyhow_to_eyre)
            .wrap_err("failed reading change info from state")?
        else {
            return Ok(None);
        };
        StoredValue::deserialize(&bytes)
            .and_then(|value| {
                storage::ChangeInfo::try_from(value).map(|info| Some(ChangeInfo::from(info)))
            })
            .wrap_err("invalid change info bytes")
    }

    #[instrument(skip_all)]
    async fn get_upgrade_change_infos(&self) -> Result<BTreeSet<ChangeInfo>> {
        self.prefix_raw(&keys::COMPONENT_PREFIX)
            .map_err(anyhow_to_eyre)
            .and_then(move |(_key, raw_value)| async move {
                StoredValue::deserialize(&raw_value)
                    .and_then(|value| storage::ChangeInfo::try_from(value).map(ChangeInfo::from))
                    .wrap_err("invalid change info bytes")
            })
            .try_collect()
            .await
    }
}

impl<T: StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_upgrade_change_info(
        &mut self,
        upgrade_name: &UpgradeName,
        change: &dyn Change,
    ) -> Result<()> {
        let change_info = change.info();
        let bytes = StoredValue::from(storage::ChangeInfo::from(&change_info))
            .serialize()
            .wrap_err("failed to serialize change info")?;
        self.put_raw(keys::change(upgrade_name, &change.name()), bytes);
        Ok(())
    }
}

impl<T: StateWrite> StateWriteExt for T {}

#[cfg(test)]
mod tests {
    use cnidarium::StateDelta;

    use super::*;

    const UPGRADE_1: UpgradeName = UpgradeName::new("up one");
    const UPGRADE_2: UpgradeName = UpgradeName::new("up two");
    const CHANGE_1: TestChange<1> = TestChange;
    const CHANGE_2: TestChange<2> = TestChange;
    const CHANGE_3: TestChange<3> = TestChange;
    const CHANGE_4: TestChange<4> = TestChange;
    const CHANGE_5: TestChange<5> = TestChange;
    const CHANGE_6: TestChange<6> = TestChange;

    #[derive(borsh::BorshSerialize)]
    struct TestChange<const N: u64>;

    impl<const N: u64> Change for TestChange<N> {
        fn name(&self) -> ChangeName {
            ChangeName::from(format!("test_change_{N}"))
        }

        fn activation_height(&self) -> u64 {
            N * 10
        }

        fn app_version(&self) -> u64 {
            N / 2 + 1
        }
    }

    #[tokio::test]
    async fn change_info_roundtrip() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        assert!(
            state
                .get_upgrade_change_info(&UPGRADE_1, &CHANGE_1.name())
                .await
                .unwrap()
                .is_none()
        );

        state
            .put_upgrade_change_info(&UPGRADE_1, &CHANGE_1)
            .unwrap();

        assert_eq!(
            state
                .get_upgrade_change_info(&UPGRADE_1, &CHANGE_1.name())
                .await
                .unwrap(),
            Some(CHANGE_1.info()),
        );
    }

    #[tokio::test]
    async fn change_info_stream() {
        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);

        for change in [&CHANGE_6 as &dyn Change, &CHANGE_5, &CHANGE_4] {
            state.put_upgrade_change_info(&UPGRADE_2, change).unwrap();
        }
        for change in [&CHANGE_3 as &dyn Change, &CHANGE_2, &CHANGE_1] {
            state.put_upgrade_change_info(&UPGRADE_1, change).unwrap();
        }

        let mut changes_iter = state.get_upgrade_change_infos().await.unwrap().into_iter();
        assert_eq!(changes_iter.next().unwrap(), CHANGE_1.info());
        assert_eq!(changes_iter.next().unwrap(), CHANGE_2.info());
        assert_eq!(changes_iter.next().unwrap(), CHANGE_3.info());
        assert_eq!(changes_iter.next().unwrap(), CHANGE_4.info());
        assert_eq!(changes_iter.next().unwrap(), CHANGE_5.info());
        assert_eq!(changes_iter.next().unwrap(), CHANGE_6.info());
        assert!(changes_iter.next().is_none());
    }
}
