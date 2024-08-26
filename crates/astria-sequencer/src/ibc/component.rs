use std::sync::Arc;

use anyhow::Result;
use astria_core::sequencer::GenesisState;
use penumbra_ibc::{
    component::Ibc,
    genesis::Content,
};
use tendermint::abci::request::{
    BeginBlock,
    EndBlock,
};
use tracing::instrument;

use crate::{
    component::Component,
    ibc::{
        host_interface::AstriaHost,
        state_ext::StateWriteExt,
    },
    storage::{
        DeltaDelta,
        DeltaDeltaCompat,
    },
};

#[derive(Default)]
pub(crate) struct IbcComponent;

impl IbcComponent {
    #[instrument(name = "IbcComponent::init_chain", skip_all)]
    pub(crate) async fn init_chain(
        state: &mut DeltaDeltaCompat,
        app_state: &GenesisState,
    ) -> Result<()> {
        state.put_ibc_sudo_address(*app_state.ibc_sudo_address());

        for address in app_state.ibc_relayer_addresses() {
            state.put_ibc_relayer_address(address);
        }

        state.put_ics20_withdrawal_base_fee(app_state.fees().ics20_withdrawal_base_fee);

        Ibc::init_chain(
            state,
            Some(&Content {
                ibc_params: app_state.ibc_params().clone(),
            }),
        )
        .await;
        Ok(())
    }

    #[instrument(name = "IbcComponent::begin_block", skip_all)]
    pub(crate) async fn begin_block(
        state: &mut Arc<DeltaDeltaCompat>,
        begin_block: &BeginBlock,
    ) -> Result<()> {
        Ibc::begin_block::<AstriaHost, _>(state, begin_block).await;
        Ok(())
    }

    #[instrument(name = "IbcComponent::end_block", skip_all)]
    pub(crate) async fn end_block(
        state: &mut Arc<DeltaDeltaCompat>,
        end_block: &EndBlock,
    ) -> Result<()> {
        Ibc::end_block(state, end_block).await;
        Ok(())
    }
}
