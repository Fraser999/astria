use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::FeeAssetChange,
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
use futures::TryStreamExt as _;
use tokio::pin;
use tracing::{
    instrument,
    Level,
};

use super::TransactionSignerAddressBytes;
use crate::{
    authority::StateReadExt as _,
    fees::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedFeeAssetChange {
    action: FeeAssetChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedFeeAssetChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: FeeAssetChange,
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
            FeeAssetChange::Addition(asset) => {
                state
                    .put_allowed_fee_asset(asset)
                    .wrap_err("failed to write allowed fee asset to storage")?;
            }
            FeeAssetChange::Removal(asset) => {
                state.delete_allowed_fee_asset(asset);
                pin!(
                    let assets = state.allowed_fee_assets();
                );
                ensure!(
                    assets
                        .try_next()
                        .await
                        .wrap_err("failed to stream fee assets from storage")?
                        .is_some(),
                    "cannot remove last allowed fee asset",
                );
            }
        }
        Ok(())
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Ensure the tx signer is the current sudo address.
        let sudo_address = state
            .get_sudo_address()
            .await
            .wrap_err("failed to read sudo address from storage")?;
        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change fee assets"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::protocol::transaction::v1::action::SudoAddressChange;

    use super::{
        super::{
            test_utils::{
                test_asset,
                Fixture,
            },
            CheckedAction,
        },
        *,
    };
    use crate::{
        authority::StateWriteExt,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            nria,
        },
    };

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Store a sudo address different from the tx signer address.
        let sudo_address = [2; ADDRESS_LEN];
        assert_ne!(fixture.tx_signer, sudo_address);
        fixture.state.put_sudo_address(sudo_address).unwrap();

        let addition_action = FeeAssetChange::Addition(test_asset());
        let err = CheckedFeeAssetChange::new(addition_action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change fee assets",
        );

        let removal_action = FeeAssetChange::Removal(test_asset());
        let err = CheckedFeeAssetChange::new(removal_action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change fee assets",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Construct the addition and removal checked actions while the sudo address is still the
        // tx signer so construction succeeds.
        let addition_action = FeeAssetChange::Addition(test_asset());
        let checked_addition_action =
            CheckedFeeAssetChange::new(addition_action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        let removal_action = FeeAssetChange::Removal(test_asset());
        let checked_removal_action =
            CheckedFeeAssetChange::new(removal_action, fixture.tx_signer, &fixture.state)
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

        // Try to execute the two checked actions now - should fail due to signer no longer being
        // authorized.
        let err = checked_addition_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change fee assets",
        );

        let err = checked_removal_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to change fee assets",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_attempting_to_remove_only_asset() {
        let mut fixture = Fixture::new().await;

        let action = FeeAssetChange::Removal(nria().into());
        let checked_action = CheckedFeeAssetChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "cannot remove last allowed fee asset");
    }

    #[tokio::test]
    async fn should_execute_addition() {
        let mut fixture = Fixture::new().await;

        let allowed_fee_assets = fixture.allowed_fee_assets().await;
        assert!(!allowed_fee_assets.contains(&test_asset().to_ibc_prefixed()));

        let action = FeeAssetChange::Addition(test_asset());
        let checked_action = CheckedFeeAssetChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        let allowed_fee_assets = fixture.allowed_fee_assets().await;
        assert!(allowed_fee_assets.contains(&test_asset().to_ibc_prefixed()));
    }

    #[tokio::test]
    async fn should_execute_removal() {
        let mut fixture = Fixture::new().await;
        fixture.state.put_allowed_fee_asset(&test_asset()).unwrap();

        let allowed_fee_assets = fixture.allowed_fee_assets().await;
        assert!(allowed_fee_assets.contains(&test_asset().to_ibc_prefixed()));

        let action = FeeAssetChange::Removal(test_asset());
        let checked_action = CheckedFeeAssetChange::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        let allowed_fee_assets = fixture.allowed_fee_assets().await;
        assert!(!allowed_fee_assets.contains(&test_asset().to_ibc_prefixed()));
    }
}
