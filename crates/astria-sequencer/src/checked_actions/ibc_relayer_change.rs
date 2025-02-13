use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::IbcRelayerChange,
};
use astria_eyre::eyre::{
    ensure,
    Result,
    WrapErr as _,
};
use cnidarium::{
    StateRead,
    StateWrite,
};
use tracing::{
    instrument,
    Level,
};

use super::TransactionSignerAddressBytes;
use crate::{
    address::StateReadExt as _,
    ibc::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedIbcRelayerChange {
    action: IbcRelayerChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedIbcRelayerChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: IbcRelayerChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // Run immutable checks for base prefix.
        match &action {
            IbcRelayerChange::Addition(address) | IbcRelayerChange::Removal(address) => {
                state
                    .ensure_base_prefix(address)
                    .await
                    .wrap_err("ibc relayer change address has an unsupported prefix")?;
            }
        }

        let checked_action = Self {
            action,
            tx_signer: tx_signer.into(),
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;

        match self.action {
            IbcRelayerChange::Addition(address) => {
                state
                    .put_ibc_relayer_address(&address)
                    .wrap_err("failed to write ibc relayer address to storage")?;
            }
            IbcRelayerChange::Removal(address) => {
                state.delete_ibc_relayer_address(&address);
            }
        }

        Ok(())
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Check that the signer of this tx is the authorized IBC sudo address.
        let ibc_sudo_address = state
            .get_ibc_sudo_address()
            .await
            .wrap_err("failed to read ibc sudo address from storage")?;
        ensure!(
            &ibc_sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change ibc relayer",
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::protocol::transaction::v1::action::IbcSudoChange;

    use super::{
        super::test_utils::{
            address_with_prefix,
            Fixture,
        },
        *,
    };
    use crate::{
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            ASTRIA_PREFIX,
        },
        checked_actions::CheckedAction,
    };

    #[tokio::test]
    async fn should_fail_construction_if_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let address = address_with_prefix([50; ADDRESS_LEN], prefix);
        let action = IbcRelayerChange::Addition(address);
        let err = CheckedIbcRelayerChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );

        let action = IbcRelayerChange::Removal(address);
        let err = CheckedIbcRelayerChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_ibc_sudo_address() {
        let mut fixture = Fixture::new().await;

        let address = astria_address(&[50; ADDRESS_LEN]);
        let addition_action = IbcRelayerChange::Addition(address);
        let removal_action = IbcRelayerChange::Removal(address);
        // Store a sudo address different from the tx signer address.
        let sudo_address = [2; ADDRESS_LEN];
        assert_ne!(fixture.tx_signer, sudo_address);
        fixture.state.put_ibc_sudo_address(sudo_address).unwrap();

        let err = CheckedIbcRelayerChange::new(addition_action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change ibc relayer",
        );

        let err = CheckedIbcRelayerChange::new(removal_action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change ibc relayer",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_bridge_sudo_address() {
        let mut fixture = Fixture::new().await;

        let address = astria_address(&[50; ADDRESS_LEN]);
        let addition_action = IbcRelayerChange::Addition(address);
        let removal_action = IbcRelayerChange::Removal(address);
        // Store the tx signer address as the sudo address.
        fixture
            .state
            .put_ibc_sudo_address(fixture.tx_signer)
            .unwrap();

        // Construct checked IBC relayer change actions while the sudo address is still the
        // tx signer so construction succeeds.
        let checked_addition_action =
            CheckedIbcRelayerChange::new(addition_action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let checked_removal_action =
            CheckedIbcRelayerChange::new(removal_action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Change the IBC sudo address to something other than the tx signer.
        let ibc_sudo_change = IbcSudoChange {
            new_address: astria_address(&[2; ADDRESS_LEN]),
        };
        let checked_ibc_sudo_change =
            CheckedAction::new_ibc_sudo_change(ibc_sudo_change, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_ibc_sudo_change
            .execute(&mut fixture.state)
            .await
            .unwrap();
        let new_ibc_sudo_address = fixture.state.get_ibc_sudo_address().await.unwrap();
        assert_ne!(fixture.tx_signer, new_ibc_sudo_address);

        // Try to execute the checked actions now - should fail due to signer no longer being
        // authorized.
        let err = checked_addition_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change ibc relayer",
        );

        let err = checked_removal_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change ibc relayer",
        );
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;
        fixture
            .state
            .put_ibc_sudo_address(fixture.tx_signer)
            .unwrap();

        let address = astria_address(&[50; ADDRESS_LEN]);
        assert!(!fixture.state.is_ibc_relayer(&address).await.unwrap());

        let addition_action = IbcRelayerChange::Addition(address);
        let checked_addition_action =
            CheckedIbcRelayerChange::new(addition_action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_addition_action
            .execute(&mut fixture.state)
            .await
            .unwrap();
        assert!(fixture.state.is_ibc_relayer(&address).await.unwrap());

        let removal_action = IbcRelayerChange::Removal(address);
        let checked_removal_action =
            CheckedIbcRelayerChange::new(removal_action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_removal_action
            .execute(&mut fixture.state)
            .await
            .unwrap();
        assert!(!fixture.state.is_ibc_relayer(&address).await.unwrap());
    }
}
