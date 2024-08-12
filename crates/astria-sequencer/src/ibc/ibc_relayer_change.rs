use anyhow::{
    ensure,
    Context as _,
    Result,
};
use astria_core::protocol::transaction::v1alpha1::action::IbcRelayerChangeAction;
use async_trait::async_trait;
use cnidarium::StateWrite;

use crate::{
    address::StateReadExt as _,
    app::ActionHandler,
    ibc::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    immutable_data::ImmutableData,
    // transaction::StateReadExt as _,
};

#[async_trait]
impl ActionHandler for IbcRelayerChangeAction {
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
        match self {
            IbcRelayerChangeAction::Addition(addr) | IbcRelayerChangeAction::Removal(addr) => {
                state
                    .ensure_base_prefix(addr, immutable_data)
                    .await
                    .context(
                        "failed check for base prefix of provided address to be added/removed",
                    )?;
            }
        }

        let ibc_sudo_address = state.get_ibc_sudo_address(immutable_data);
        ensure!(
            ibc_sudo_address == from,
            "unauthorized address for IBC relayer change"
        );

        match self {
            IbcRelayerChangeAction::Addition(address) => {
                state.put_ibc_relayer_address(address);
            }
            IbcRelayerChangeAction::Removal(address) => {
                state.delete_ibc_relayer_address(address);
            }
        }
        Ok(())
    }
}
