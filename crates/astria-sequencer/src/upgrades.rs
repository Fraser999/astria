use std::path::Path;
use std::sync::Arc;
use astria_core::generated::upgrades::v1::{
    ConnectOracleUpgrade,
    Upgrades as RawUpgrades,
    WhateverUpgrade,
};
use astria_eyre::eyre::{
    Result,
    WrapErr,
};
use cnidarium::StateWrite;

trait Upgrade {
    fn activation_height(&self) -> u64;
    fn shutdown_required(&self) -> bool;
}

pub(crate) struct Upgrades {
    connect_oracle: Option<Arc<ConnectOracleUpgrade>>,
    whatever: Option<Arc<WhateverUpgrade>>,
}

impl Upgrades {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .wrap_err_with(|| format!("failed to read {}", path.display()))?;
        let upgrades = serde_json::from_str::<RawUpgrades>(&contents)
            .wrap_err_with(|| format!("failed to parse {}", path.display()))?;
        Ok(Self {
            connect_oracle: upgrades.connect_oracle,
            whatever: upgrades.whatever,
        })
    }

    pub(crate) fn apply_upgrade_if_due_now<S: StateWrite>(&self, mut state: S) -> Result<()> {
        todo!()
    }
}
