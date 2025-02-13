use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::BridgeSudoChange,
};
use astria_eyre::eyre::{
    bail,
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
    bridge::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedBridgeSudoChange {
    action: BridgeSudoChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedBridgeSudoChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: BridgeSudoChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // Immutable checks for base prefix.
        //
        // TODO(Fraser): is this first check necessary?  We call `get_bridge_account_sudo_address`
        //               in the mutable checks.
        state
            .ensure_base_prefix(&action.bridge_address)
            .await
            .wrap_err("bridge address has an unsupported prefix")?;
        if let Some(new_sudo_address) = &action.new_sudo_address {
            state
                .ensure_base_prefix(new_sudo_address)
                .await
                .wrap_err("new sudo address has an unsupported prefix")?;
        }
        if let Some(new_withdrawer_address) = &action.new_withdrawer_address {
            state
                .ensure_base_prefix(new_withdrawer_address)
                .await
                .wrap_err("new withdrawer address has an unsupported prefix")?;
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

        if let Some(sudo_address) = self.action.new_sudo_address {
            state
                .put_bridge_account_sudo_address(&self.action.bridge_address, sudo_address)
                .wrap_err("failed to write bridge account sudo address to storage")?;
        }

        if let Some(withdrawer_address) = self.action.new_withdrawer_address {
            state
                .put_bridge_account_withdrawer_address(
                    &self.action.bridge_address,
                    withdrawer_address,
                )
                .wrap_err("failed to write bridge account withdrawer address to storage")?;
        }

        Ok(())
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // check that the signer of this tx is the authorized sudo address for the bridge account
        let Some(sudo_address) = state
            .get_bridge_account_sudo_address(&self.action.bridge_address)
            .await
            .wrap_err("failed to read bridge account sudo address from storage")?
        else {
            // TODO: if the sudo address is unset, should we still allow this action
            // if the signer is the bridge address itself?
            bail!("bridge account does not have an associated sudo address in storage");
        };

        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change bridge sudo address",
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::protocol::transaction::v1::action::BridgeSudoChange;

    use super::{
        super::test_utils::{
            address_with_prefix,
            Fixture,
        },
        *,
    };
    use crate::benchmark_and_test_utils::{
        assert_eyre_error,
        astria_address,
        ASTRIA_PREFIX,
    };

    fn new_bridge_sudo_change() -> BridgeSudoChange {
        BridgeSudoChange {
            bridge_address: astria_address(&[99; ADDRESS_LEN]),
            new_sudo_address: Some(astria_address(&[98; ADDRESS_LEN])),
            new_withdrawer_address: Some(astria_address(&[97; ADDRESS_LEN])),
            fee_asset: "test".parse().unwrap(),
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeSudoChange {
            bridge_address: address_with_prefix([50; ADDRESS_LEN], prefix),
            ..new_bridge_sudo_change()
        };
        let err = CheckedBridgeSudoChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_new_sudo_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeSudoChange {
            new_sudo_address: Some(address_with_prefix([50; ADDRESS_LEN], prefix)),
            ..new_bridge_sudo_change()
        };
        let err = CheckedBridgeSudoChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_new_withdrawer_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeSudoChange {
            new_withdrawer_address: Some(address_with_prefix([50; ADDRESS_LEN], prefix)),
            ..new_bridge_sudo_change()
        };
        let err = CheckedBridgeSudoChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_sudo_address_not_set() {
        let fixture = Fixture::new().await;

        let action = new_bridge_sudo_change();
        let err = CheckedBridgeSudoChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "bridge account does not have an associated sudo address",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_bridge_sudo_address() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_sudo_change();
        // Store a sudo address different from the tx signer address.
        let sudo_address = [2; ADDRESS_LEN];
        assert_ne!(fixture.tx_signer, sudo_address);
        fixture
            .state
            .put_bridge_account_sudo_address(&action.bridge_address, sudo_address)
            .unwrap();

        let err = CheckedBridgeSudoChange::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to change bridge sudo address",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_bridge_sudo_address() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_sudo_change();
        let bridge_address = action.bridge_address;
        // Store the tx signer address as the sudo address.
        fixture
            .state
            .put_bridge_account_sudo_address(&bridge_address, fixture.tx_signer)
            .unwrap();

        // Construct two checked bridge sudo change actions while the sudo address is still the
        // tx signer so construction succeeds.
        let checked_action_1 =
            CheckedBridgeSudoChange::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let checked_action_2 =
            CheckedBridgeSudoChange::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first checked action to change the sudo address to one different from the tx
        // signer address.
        checked_action_1.execute(&mut fixture.state).await.unwrap();
        let new_sudo_address = fixture
            .state
            .get_bridge_account_sudo_address(&bridge_address)
            .await
            .expect("should get bridge sudo address")
            .expect("bridge sudo address should be Some");
        assert_ne!(fixture.tx_signer, new_sudo_address);

        // Try to execute the second checked action now - should fail due to signer no longer being
        // authorized.
        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to change bridge sudo address",
        );
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_sudo_change();
        let bridge_address = action.bridge_address;
        let new_sudo_address = action.new_sudo_address.unwrap();
        let new_withdrawer_address = action.new_withdrawer_address.unwrap();
        fixture
            .state
            .put_bridge_account_sudo_address(&bridge_address, fixture.tx_signer)
            .unwrap();
        let checked_action =
            CheckedBridgeSudoChange::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        assert_eq!(
            fixture
                .state
                .get_bridge_account_sudo_address(&bridge_address)
                .await
                .unwrap(),
            Some(new_sudo_address.bytes()),
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_withdrawer_address(&bridge_address)
                .await
                .unwrap(),
            Some(new_withdrawer_address.bytes()),
        );
    }
}
