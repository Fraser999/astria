use anyhow::{
    bail,
    ensure,
    Context as _,
    Result,
};
use astria_core::protocol::transaction::v1alpha1::action::FeeAssetChangeAction;
use async_trait::async_trait;
use cnidarium::StateWrite;

use crate::{
    app::ActionHandler,
    assets::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    authority::StateReadExt as _,
    immutable_data::ImmutableData,
    // transaction::StateReadExt as _,
};

#[async_trait]
impl ActionHandler for FeeAssetChangeAction {
    type CheckStatelessContext = ();

    async fn check_stateless(&self, _context: Self::CheckStatelessContext) -> Result<()> {
        Ok(())
    }

    async fn check_and_execute<S: StateWrite>(
        &self,
        mut state: S,
        immutable_data: &ImmutableData,
        from: [u8; 20],
    ) -> Result<()> {
        let authority_sudo_address = state.get_sudo_address(immutable_data);
        ensure!(
            authority_sudo_address == from,
            "unauthorized address for fee asset change"
        );
        match self {
            FeeAssetChangeAction::Addition(asset) => {
                state.put_allowed_fee_asset(asset);
            }
            FeeAssetChangeAction::Removal(asset) => {
                state.delete_allowed_fee_asset(asset);

                if state
                    .get_allowed_fee_assets()
                    .await
                    .context("failed to retrieve allowed fee assets")?
                    .is_empty()
                {
                    bail!("cannot remove last allowed fee asset");
                }
            }
        }
        Ok(())
    }
}
