use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::Transfer,
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
    accounts::StateWriteExt,
    address::StateReadExt,
    bridge::StateReadExt as _,
};

#[derive(Debug)]
pub(crate) struct CheckedTransfer {
    action: Transfer,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedTransfer {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: Transfer,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // Run immutable checks for base prefix.
        state
            .ensure_base_prefix(&action.to)
            .await
            .wrap_err("destination address has an unsupported prefix")?;

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
            .decrease_balance(&self.tx_signer, &self.action.asset, self.action.amount)
            .await
            .wrap_err("failed to decrease signer account balance")?;
        state
            .increase_balance(&self.action.to, &self.action.asset, self.action.amount)
            .await
            .wrap_err("failed to increase destination account balance")?;

        Ok(())
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Ensure the tx signer account is not a bridge account.
        ensure!(
            state
                .get_bridge_account_rollup_id(&self.tx_signer)
                .await
                .wrap_err("failed to read bridge account rollup id from storage")?
                .is_none(),
            "cannot transfer out of bridge account; BridgeUnlock or BridgeTransfer must be used",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::{
        primitive::v1::RollupId,
        protocol::transaction::v1::action::*,
    };

    use super::{
        super::{
            test_utils::{
                address_with_prefix,
                Fixture,
            },
            CheckedAction,
        },
        *,
    };
    use crate::{
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            nria,
            ASTRIA_PREFIX,
        },
        bridge::StateWriteExt as _,
    };

    fn new_transfer() -> Transfer {
        Transfer {
            to: astria_address(&[50; ADDRESS_LEN]),
            fee_asset: nria().into(),
            asset: nria().into(),
            amount: 100,
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = Transfer {
            to: address_with_prefix([50; ADDRESS_LEN], prefix),
            ..new_transfer()
        };
        let err = CheckedTransfer::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_account_is_bridge_account() {
        let mut fixture = Fixture::new().await;
        fixture
            .state
            .put_bridge_account_rollup_id(&fixture.tx_signer, RollupId::new([1u8; 32]))
            .unwrap();

        let err = CheckedTransfer::new(new_transfer(), fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "cannot transfer out of bridge account; BridgeUnlock or BridgeTransfer must be used",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_account_is_bridge_account() {
        let mut fixture = Fixture::new().await;

        // Construct a checked transfer while the signer account is not a bridge account.
        let action = new_transfer();
        let checked_action =
            CheckedTransfer::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Initialize the signer's account as a bridge account.
        let init_bridge_account = InitBridgeAccount {
            rollup_id: RollupId::new([1; 32]),
            asset: "test".parse().unwrap(),
            fee_asset: "test".parse().unwrap(),
            sudo_address: None,
            withdrawer_address: None,
        };
        let checked_init_bridge_account = CheckedAction::new_init_bridge_account(
            init_bridge_account,
            fixture.tx_signer,
            &fixture.state,
        )
        .await
        .unwrap();
        checked_init_bridge_account
            .execute(&mut fixture.state)
            .await
            .unwrap();

        // Try to execute the checked transfer now - should fail due to bridge account now existing.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "cannot transfer out of bridge account; BridgeUnlock or BridgeTransfer must be used",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_account_has_insufficient_balance() {
        let mut fixture = Fixture::new().await;

        let action = new_transfer();
        let checked_action = CheckedTransfer::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "failed to decrease signer account balance");
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;

        // Construct the checked transfer while the account has insufficient balance to ensure
        // balance checks are only part of execution.
        let action = new_transfer();
        let checked_action =
            CheckedTransfer::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Provide the signer account with sufficient balance.
        fixture
            .state
            .increase_balance(&fixture.tx_signer, &action.asset, action.amount)
            .await
            .unwrap();

        // Check the balances are correct before execution.
        assert_eq!(
            fixture.get_nria_balance(&fixture.tx_signer).await,
            action.amount
        );
        assert_eq!(fixture.get_nria_balance(&action.to).await, 0);

        // Execute the transfer.
        checked_action.execute(&mut fixture.state).await.unwrap();

        assert_eq!(fixture.get_nria_balance(&fixture.tx_signer).await, 0);
        assert_eq!(fixture.get_nria_balance(&action.to).await, action.amount);
    }
}
