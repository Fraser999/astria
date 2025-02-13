use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::FeeChange,
};
use astria_eyre::{
    eyre::{
        ensure,
        WrapErr as _,
    },
    Result,
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
    authority::StateReadExt as _,
    fees::StateWriteExt as _,
};

#[derive(Debug)]
pub(crate) struct CheckedFeeChange {
    action: FeeChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedFeeChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: FeeChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
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

        match &self.action {
            FeeChange::Transfer(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write transfer fees to storage"),
            FeeChange::RollupDataSubmission(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write sequence fees to storage"),
            FeeChange::Ics20Withdrawal(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write ics20 withdrawal fees to storage"),
            FeeChange::InitBridgeAccount(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write init bridge account fees to storage"),
            FeeChange::BridgeLock(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write bridge lock fees to storage"),
            FeeChange::BridgeUnlock(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write bridge unlock fees to storage"),
            FeeChange::BridgeSudoChange(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write bridge sudo change fees to storage"),
            FeeChange::IbcRelay(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write ibc relay fees to storage"),
            FeeChange::ValidatorUpdate(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write validator update fees to storage"),
            FeeChange::FeeAssetChange(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write fee asset change fees to storage"),
            FeeChange::FeeChange(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write fee change fees to storage"),
            FeeChange::IbcRelayerChange(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write ibc relayer change fees to storage"),
            FeeChange::SudoAddressChange(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write sudo address change fees to storage"),
            FeeChange::IbcSudoChange(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write ibc sudo change fees to storage"),
            FeeChange::BridgeTransfer(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write bridge transfer fees to storage"),
            FeeChange::RecoverIbcClient(fees) => state
                .put_fees(*fees)
                .wrap_err("failed to write recover ibc client fees to storage"),
        }
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Ensure the tx signer is the current sudo address.
        let sudo_address = state
            .get_sudo_address()
            .await
            .wrap_err("failed to read sudo address from storage")?;
        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change fees"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use astria_core::protocol::{
        fees::v1::*,
        transaction::v1::action::{
            SudoAddressChange,
            *,
        },
    };
    use astria_eyre::eyre::Report;
    use penumbra_ibc::IbcRelay;

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
        },
        fees::{
            FeeHandler,
            StateReadExt as _,
        },
        storage::StoredValue,
    };

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Store a sudo address different from the tx signer address.
        let sudo_address = [2; ADDRESS_LEN];
        assert_ne!(fixture.tx_signer, sudo_address);
        fixture.state.put_sudo_address(sudo_address).unwrap();

        let action = FeeChange::Transfer(FeeComponents::<Transfer>::new(1, 2));
        let err = CheckedFeeChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "transaction signer not authorized to change fees");
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Construct the checked action while the sudo address is still the tx signer so
        // construction succeeds.
        let action = FeeChange::Transfer(FeeComponents::<Transfer>::new(1, 2));
        let checked_action = CheckedFeeChange::new(action, fixture.tx_signer, &fixture.state)
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
        assert_eyre_error(&err, "transaction signer not authorized to change fees");
    }

    #[tokio::test]
    async fn should_execute_transfer_fee_change() {
        test_fee_change_action::<Transfer>().await;
    }

    #[tokio::test]
    async fn should_execute_rollup_data_submission_fee_change() {
        test_fee_change_action::<RollupDataSubmission>().await;
    }

    #[tokio::test]
    async fn should_execute_ics20_withdrawal_fee_change() {
        test_fee_change_action::<Ics20Withdrawal>().await;
    }

    #[tokio::test]
    async fn should_execute_init_bridge_account_fee_change() {
        test_fee_change_action::<InitBridgeAccount>().await;
    }

    #[tokio::test]
    async fn should_execute_bridge_lock_fee_change() {
        test_fee_change_action::<BridgeLock>().await;
    }

    #[tokio::test]
    async fn should_execute_bridge_unlock_fee_change() {
        test_fee_change_action::<BridgeUnlock>().await;
    }

    #[tokio::test]
    async fn should_execute_bridge_sudo_change_fee_change() {
        test_fee_change_action::<BridgeSudoChange>().await;
    }

    #[tokio::test]
    async fn should_execute_ibc_relay_fee_change() {
        test_fee_change_action::<IbcRelay>().await;
    }

    #[tokio::test]
    async fn should_execute_validator_update_fee_change() {
        test_fee_change_action::<ValidatorUpdate>().await;
    }

    #[tokio::test]
    async fn should_execute_fee_asset_change_fee_change() {
        test_fee_change_action::<FeeAssetChange>().await;
    }

    #[tokio::test]
    async fn should_execute_fee_change_fee_change() {
        test_fee_change_action::<FeeChange>().await;
    }

    #[tokio::test]
    async fn should_execute_ibc_relayer_change_fee_change() {
        test_fee_change_action::<IbcRelayerChange>().await;
    }

    #[tokio::test]
    async fn should_execute_sudo_address_change_fee_change() {
        test_fee_change_action::<SudoAddressChange>().await;
    }

    #[tokio::test]
    async fn should_execute_ibc_sudo_change_fee_change() {
        test_fee_change_action::<IbcSudoChange>().await;
    }

    #[tokio::test]
    async fn should_execute_bridge_transfer_fee_change() {
        test_fee_change_action::<BridgeTransfer>().await;
    }

    #[tokio::test]
    async fn should_execute_recover_ibc_client_fee_change() {
        test_fee_change_action::<RecoverIbcClient>().await;
    }

    async fn test_fee_change_action<'a, F>()
    where
        F: FeeHandler,
        FeeComponents<F>: TryFrom<StoredValue<'a>, Error = Report> + Debug,
        FeeChange: From<FeeComponents<F>>,
    {
        let mut fixture = Fixture::new().await;

        assert!(fixture
            .state
            .get_fees::<F>()
            .await
            .expect("should not error fetching unstored action fees")
            .is_none());

        // Execute an initial fee change tx to store the first version of the fees.
        let initial_fees = FeeComponents::<F>::new(1, 2);
        let action = FeeChange::from(initial_fees);
        CheckedFeeChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .expect("should construct checked fee change action")
            .execute(&mut fixture.state)
            .await
            .expect("should execute checked fee change action");

        let retrieved_fees = fixture
            .state
            .get_fees::<F>()
            .await
            .expect("should not error fetching initial action fees")
            .expect("initial action fees should be stored");
        assert_eq!(initial_fees, retrieved_fees);

        // Execute a second fee change tx to overwrite the fees.
        let new_fees = FeeComponents::<F>::new(3, 4);
        let new_action = FeeChange::from(new_fees);
        CheckedFeeChange::new(new_action, fixture.tx_signer, &fixture.state)
            .await
            .expect("should construct checked fee change action")
            .execute(&mut fixture.state)
            .await
            .expect("should execute checked fee change action");

        let retrieved_fees = fixture
            .state
            .get_fees::<F>()
            .await
            .expect("should not error fetching new action fees")
            .expect("new action fees should be stored");
        assert_ne!(initial_fees, retrieved_fees);
        assert_eq!(new_fees, retrieved_fees);
    }
}
