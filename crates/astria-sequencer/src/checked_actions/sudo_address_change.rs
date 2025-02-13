use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::SudoAddressChange,
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
    authority::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedSudoAddressChange {
    action: SudoAddressChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedSudoAddressChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: SudoAddressChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        state
            .ensure_base_prefix(&action.new_address)
            .await
            .wrap_err("new sudo address has an unsupported prefix")?;
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
            .put_sudo_address(self.action.new_address)
            .wrap_err("failed to write new sudo address to storage")
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Ensure the tx signer is the current sudo address.
        let sudo_address = state
            .get_sudo_address()
            .await
            .wrap_err("failed to read sudo address from storage")?;
        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change sudo address",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::primitive::v1::Address;

    use super::{
        super::test_utils::Fixture,
        *,
    };
    use crate::benchmark_and_test_utils::{
        assert_eyre_error,
        astria_address,
        ASTRIA_PREFIX,
    };

    #[tokio::test]
    async fn should_fail_construction_if_new_sudo_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let new_address = Address::builder()
            .array([50; ADDRESS_LEN])
            .prefix(prefix)
            .try_build()
            .unwrap();

        let action = SudoAddressChange {
            new_address,
        };
        let err = CheckedSudoAddressChange::new(action, fixture.tx_signer, fixture.state)
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

        let action = SudoAddressChange {
            new_address: astria_address(&[3; ADDRESS_LEN]),
        };
        let err = CheckedSudoAddressChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change sudo address",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Construct two checked sudo address change actions while the sudo address is still the tx
        // signer so construction succeeds.
        let action = SudoAddressChange {
            new_address: astria_address(&[2; ADDRESS_LEN]),
        };
        let checked_action_1 =
            CheckedSudoAddressChange::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let checked_action_2 =
            CheckedSudoAddressChange::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first checked action to change the sudo address to one different from the tx
        // signer address.
        checked_action_1.execute(&mut fixture.state).await.unwrap();
        let new_sudo_address = fixture.state.get_sudo_address().await.unwrap();
        assert_ne!(fixture.tx_signer, new_sudo_address);

        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to change sudo address",
        );
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;

        let old_sudo_address = fixture.state.get_sudo_address().await.unwrap();
        let new_sudo_address = astria_address(&[2; ADDRESS_LEN]);
        assert_ne!(old_sudo_address, new_sudo_address.bytes());

        let action = SudoAddressChange {
            new_address: new_sudo_address,
        };
        let checked_action =
            CheckedSudoAddressChange::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();
        let sudo_address = fixture.state.get_sudo_address().await.unwrap();
        assert_eq!(sudo_address, new_sudo_address.bytes());
    }
}
