use astria_core::upgrades::v1::{Change, Upgrade1};
use astria_eyre::{
    anyhow_to_eyre,
    eyre::{
        OptionExt,
        Result,
        WrapErr,
    },
};
use async_trait::async_trait;
use cnidarium::Snapshot;
use astria_eyre::eyre::bail;
use crate::app::{
    ShouldShutDown,
    StateReadExt as _,
};
use super::state_ext::StateReadExt as _;

#[async_trait]
pub(crate) trait Upgrade {
    fn activation_height(&self) -> u64;

    fn shutdown_required(&self) -> bool;

    fn name(&self) -> &'static str;


    fn changes(&self) -> impl Iterator<Item = &'_ dyn Change>;

    async fn should_shut_down(&self, snapshot: &Snapshot) -> Result<ShouldShutDown> {
        if !self.shutdown_required() {
            return Ok(ShouldShutDown::ContinueRunning);
        }

        if next_block_height(snapshot).await? != self.activation_height() {
            return Ok(ShouldShutDown::ContinueRunning);
        }

        let block_time = snapshot
            .get_block_timestamp()
            .await
            .wrap_err("failed getting latest block time from snapshot")?;
        let app_hash = snapshot
            .root_hash()
            .await
            .map_err(anyhow_to_eyre)
            .wrap_err("failed to get current root hash from snapshot")?;
        let hex_encoded_app_hash = hex::encode(&app_hash.0);
        Ok(ShouldShutDown::ShutDownForUpgrade {
            upgrade_activation_height: self.activation_height(),
            block_time,
            hex_encoded_app_hash,
        })
    }

    async fn ensure_historical_upgrades_applied(&self, snapshot: &Snapshot) -> Result<()> {
        if next_block_height(snapshot).await? >= self.activation_height() {
            return Ok(());
        }

        for change in self.changes() {
            let Some(stored_change_hash) = snapshot.get_upgrade_change_hash(self, change).await.wrap_err("failed to get upgrade change hash")? else {
                bail!(
                    "historical upgrade change `{}/{}` has not been applied (wrong upgrade.json \
                    file provided?)",
                    self.name(), change.name()
                );
            };
            let actual_hash = change.calculate_hash();
            if actual_hash != stored_change_hash {
                bail!(
                    "upgrade change hash `{actual_hash}` does not match stored hash \
                    `{stored_change_hash}` for `{}/{}`",
                    self.name(), change.name()
                );
            }
        }

        Ok(())
    }
}

async fn next_block_height(snapshot: &Snapshot) -> Result<u64> {
    snapshot
        .get_block_height()
        .await
        .unwrap_or_default()
        .checked_add(1)
        .ok_or_eyre("overflowed getting next block height")
}

impl Upgrade for Upgrade1 {
    fn activation_height(&self) -> u64 {
        self.activation_height()
    }

    fn shutdown_required(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "upgrade_1"
    }

    fn changes(&self) -> impl Iterator<Item = &'_ dyn Change> {
        Some(&self.connect_oracle_change() as &dyn Change)
            .into_iter()
            .chain(Some(self.connect_oracle_change() as &dyn Change))
    }
}
