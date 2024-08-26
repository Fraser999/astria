use anyhow::{
    Context,
    Result,
};
use tendermint::abci::request::{
    BeginBlock,
    EndBlock,
};
use tracing::instrument;

use crate::{
    accounts::StateWriteExt as _,
    assets::StateReadExt as _,
    component::Component,
    storage::DeltaDelta,
};

#[derive(Default)]
pub(crate) struct AccountsComponent;

#[async_trait::async_trait]
impl Component for AccountsComponent {
    type AppState = astria_core::sequencer::GenesisState;

    #[instrument(name = "AccountsComponent::init_chain", skip_all)]
    async fn init_chain(state: &DeltaDelta, app_state: &Self::AppState) -> Result<()> {
        let native_asset = state
            .get_native_asset()
            .await
            .context("failed to read native asset from state")?;
        for account in app_state.accounts() {
            state.put_account_balance(account.address, &native_asset, account.balance);
        }

        state.put_transfer_base_fee(app_state.fees().transfer_base_fee);
        Ok(())
    }

    #[instrument(name = "AccountsComponent::begin_block", skip_all)]
    async fn begin_block(_state: &DeltaDelta, _begin_block: &BeginBlock) -> Result<()> {
        Ok(())
    }

    #[instrument(name = "AccountsComponent::end_block", skip_all)]
    async fn end_block(_state: &DeltaDelta, _end_block: &EndBlock) -> Result<()> {
        Ok(())
    }
}
