use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::IbcSudoChange,
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
    authority::StateReadExt as _,
    ibc::StateWriteExt as _,
};

#[derive(Debug)]
pub(crate) struct CheckedIbcSudoChange {
    action: IbcSudoChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedIbcSudoChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: IbcSudoChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // Run immutable checks for base prefix.
        state
            .ensure_base_prefix(&action.new_address)
            .await
            .wrap_err("new ibc sudo address has an unsupported prefix")?;

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
        state
            .put_ibc_sudo_address(self.action.new_address)
            .wrap_err("failed to write ibc sudo address to storage")
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Check that the signer of this tx is the authorized sudo address.
        let sudo_address = state
            .get_sudo_address()
            .await
            .wrap_err("failed to read sudo address from storage")?;
        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change ibc sudo address",
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::{
        primitive::v1::Address,
        protocol::transaction::v1::action::SudoAddressChange,
    };

    use super::{
        super::{
            test_utils::Fixture,
            CheckedAction,
        },
        *,
    };
    use crate::{
        authority::StateWriteExt as _,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            ASTRIA_PREFIX,
        },
        ibc::StateReadExt as _,
    };

    #[tokio::test]
    async fn should_fail_construction_if_new_ibc_sudo_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let new_address = Address::builder()
            .array([50; ADDRESS_LEN])
            .prefix(prefix)
            .try_build()
            .unwrap();

        let action = IbcSudoChange {
            new_address,
        };
        let err = CheckedIbcSudoChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Store a sudo address different from the tx signer address.
        let sudo_address = [2; ADDRESS_LEN];
        assert_ne!(fixture.tx_signer, sudo_address);
        fixture.state.put_sudo_address(sudo_address).unwrap();

        let action = IbcSudoChange {
            new_address: astria_address(&[3; ADDRESS_LEN]),
        };
        let err = CheckedIbcSudoChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change ibc sudo address",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Construct a checked IBC sudo address change action while the sudo address is still the tx
        // signer so construction succeeds.
        let action = IbcSudoChange {
            new_address: astria_address(&[2; ADDRESS_LEN]),
        };
        let checked_action =
            CheckedIbcSudoChange::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        // Change the sudo address to something other than the tx signer.
        let sudo_address_change = SudoAddressChange {
            new_address: astria_address(&[2; ADDRESS_LEN]),
        };
        let checked_sudo_address_change = CheckedAction::new_sudo_address_change(
            sudo_address_change,
            fixture.tx_signer,
            &fixture.state,
        )
        .await
        .unwrap();
        checked_sudo_address_change
            .execute(&mut fixture.state)
            .await
            .unwrap();
        let new_sudo_address = fixture.state.get_sudo_address().await.unwrap();
        assert_ne!(fixture.tx_signer, new_sudo_address);

        // Try to execute the checked action now - should fail due to signer no longer being
        // authorized.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to change ibc sudo address",
        );
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;
        let old_ibc_sudo_address = astria_address(&[1; ADDRESS_LEN]);
        fixture
            .state
            .put_ibc_sudo_address(old_ibc_sudo_address)
            .unwrap();

        let new_ibc_sudo_address = astria_address(&[2; ADDRESS_LEN]);
        assert_ne!(old_ibc_sudo_address, new_ibc_sudo_address);

        let action = IbcSudoChange {
            new_address: new_ibc_sudo_address,
        };
        let checked_action = CheckedIbcSudoChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();
        let ibc_sudo_address = fixture.state.get_ibc_sudo_address().await.unwrap();
        assert_eq!(ibc_sudo_address, new_ibc_sudo_address.bytes());
    }
}
