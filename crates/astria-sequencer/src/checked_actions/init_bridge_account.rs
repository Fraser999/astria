use astria_core::{
    primitive::v1::{
        Address,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::InitBridgeAccount,
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
    bridge::{
        StateReadExt as _,
        StateWriteExt,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedInitBridgeAccount {
    action: InitBridgeAccount,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedInitBridgeAccount {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: InitBridgeAccount,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // Immutable checks for base prefix.
        //
        // TODO(Fraser): Should we do `ensure_base_prefix(tx_signer)`? Note, that would be a
        //               consensus breaking change.
        if let Some(sudo_address) = &action.sudo_address {
            state
                .ensure_base_prefix(sudo_address)
                .await
                .wrap_err("sudo address has an unsupported prefix")?;
        }
        if let Some(withdrawer_address) = &action.withdrawer_address {
            state
                .ensure_base_prefix(withdrawer_address)
                .await
                .wrap_err("withdrawer address has an unsupported prefix")?;
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
        state
            .put_bridge_account_rollup_id(&self.tx_signer, self.action.rollup_id)
            .wrap_err("failed to write bridge account rollup id to storage")?;
        state
            .put_bridge_account_ibc_asset(&self.tx_signer, &self.action.asset)
            .wrap_err("failed to write bridge account asset to storage")?;
        state
            .put_bridge_account_sudo_address(
                &self.tx_signer,
                self.action
                    .sudo_address
                    .map_or(*self.tx_signer.as_bytes(), Address::bytes),
            )
            .wrap_err("failed to write bridge account sudo address to storage")?;
        state
            .put_bridge_account_withdrawer_address(
                &self.tx_signer,
                self.action
                    .withdrawer_address
                    .map_or(*self.tx_signer.as_bytes(), Address::bytes),
            )
            .wrap_err("failed to write bridge account withdrawer address to storage")
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Ensure the tx signer account is not a bridge account.
        //
        // This prevents the address from being registered as a bridge account if it's been
        // previously initialized as a bridge account.
        //
        // However, there is no prevention of initializing an account as a bridge account that's
        // already been used as a normal EOA.
        //
        // The implication is that the account might already have a balance, nonce, etc. before
        // being converted into a bridge account.
        //
        // After the account becomes a bridge account, it can no longer receive funds via
        // `Transfer`, only via `BridgeLock` or `BridgeTransfer`.
        ensure!(
            state
                .get_bridge_account_rollup_id(&self.tx_signer)
                .await
                .wrap_err("failed to read bridge account rollup id from storage")?
                .is_none(),
            "bridge account already exists",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::primitive::v1::RollupId;

    use super::{
        super::test_utils::{
            address_with_prefix,
            Fixture,
        },
        *,
    };
    use crate::{
        accounts::AddressBytes as _,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            ASTRIA_PREFIX,
        },
    };

    fn new_init_bridge_account() -> InitBridgeAccount {
        InitBridgeAccount {
            rollup_id: RollupId::new([1; 32]),
            asset: "test".parse().unwrap(),
            fee_asset: "test".parse().unwrap(),
            sudo_address: Some(astria_address(&[2; ADDRESS_LEN])),
            withdrawer_address: Some(astria_address(&[3; ADDRESS_LEN])),
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_new_sudo_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = InitBridgeAccount {
            sudo_address: Some(address_with_prefix([2; ADDRESS_LEN], prefix)),
            ..new_init_bridge_account()
        };
        let err = CheckedInitBridgeAccount::new(action, fixture.tx_signer, fixture.state)
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
        let action = InitBridgeAccount {
            withdrawer_address: Some(address_with_prefix([3; ADDRESS_LEN], prefix)),
            ..new_init_bridge_account()
        };
        let err = CheckedInitBridgeAccount::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_account_already_initialized() {
        let mut fixture = Fixture::new().await;
        fixture
            .state
            .put_bridge_account_rollup_id(&fixture.tx_signer, RollupId::new([1u8; 32]))
            .unwrap();

        let action = new_init_bridge_account();
        let err = CheckedInitBridgeAccount::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "bridge account already exists");
    }

    #[tokio::test]
    async fn should_fail_execution_if_bridge_account_already_initialized() {
        let mut fixture = Fixture::new().await;

        // Construct two checked init bridge account actions while the bridge account doesn't
        // exist so construction succeeds.
        let action = new_init_bridge_account();
        let checked_action_1 =
            CheckedInitBridgeAccount::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let checked_action_2 =
            CheckedInitBridgeAccount::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first checked action to initialize the bridge account.
        checked_action_1.execute(&mut fixture.state).await.unwrap();

        // Try to execute the second checked action now - should fail due to bridge account now
        // existing.
        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "bridge account already exists");
    }

    #[tokio::test]
    async fn should_execute_using_sudo_address_and_withdrawer_address() {
        let mut fixture = Fixture::new().await;

        let action = new_init_bridge_account();
        let checked_action =
            CheckedInitBridgeAccount::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        assert_eq!(
            fixture
                .state
                .get_bridge_account_rollup_id(&fixture.tx_signer)
                .await
                .unwrap(),
            Some(action.rollup_id)
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_ibc_asset(&fixture.tx_signer)
                .await
                .unwrap(),
            action.asset.to_ibc_prefixed()
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_sudo_address(&fixture.tx_signer)
                .await
                .unwrap(),
            Some(*action.sudo_address.unwrap().address_bytes())
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_withdrawer_address(&fixture.tx_signer)
                .await
                .unwrap(),
            Some(*action.withdrawer_address.unwrap().address_bytes())
        );
    }

    #[tokio::test]
    async fn should_execute_using_no_sudo_address_and_no_withdrawer_address() {
        let mut fixture = Fixture::new().await;

        let action = InitBridgeAccount {
            sudo_address: None,
            withdrawer_address: None,
            ..new_init_bridge_account()
        };
        let checked_action =
            CheckedInitBridgeAccount::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        assert_eq!(
            fixture
                .state
                .get_bridge_account_rollup_id(&fixture.tx_signer)
                .await
                .unwrap(),
            Some(action.rollup_id)
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_ibc_asset(&fixture.tx_signer)
                .await
                .unwrap(),
            action.asset.to_ibc_prefixed()
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_sudo_address(&fixture.tx_signer)
                .await
                .unwrap(),
            Some(fixture.tx_signer)
        );
        assert_eq!(
            fixture
                .state
                .get_bridge_account_withdrawer_address(&fixture.tx_signer)
                .await
                .unwrap(),
            Some(fixture.tx_signer)
        );
    }
}
