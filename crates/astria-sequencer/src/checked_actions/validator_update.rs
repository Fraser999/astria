use astria_core::{
    primitive::v1::{
        asset::IbcPrefixed,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::ValidatorUpdate,
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

use super::{
    AssetTransfer,
    TransactionSignerAddressBytes,
};
use crate::authority::{
    StateReadExt as _,
    StateWriteExt as _,
};

#[derive(Debug)]
pub(crate) struct CheckedValidatorUpdate {
    action: ValidatorUpdate,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedValidatorUpdate {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: ValidatorUpdate,
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
    pub(super) async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Check that the signer of this tx is the authorized sudo address.
        let sudo_address = state
            .get_sudo_address()
            .await
            .wrap_err("failed to read sudo address from storage")?;
        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to update validator set",
        );

        // Ensure that we're not removing the last validator or a validator that doesn't exist;
        // these both cause issues in CometBFT.
        if self.action.power == 0 {
            let validator_set = state
                .get_validator_set()
                .await
                .wrap_err("failed to read validator set from storage")?;
            // Check that validator exists.
            if validator_set.get(&self.action.verification_key).is_none() {
                bail!("cannot remove a non-existing validator");
            }
            // Check that this is not the only validator, cannot remove the last one.
            ensure!(validator_set.len() != 1, "cannot remove the only validator");
        }

        Ok(())
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;

        // Add validator update in nonverifiable state to be used in end_block.
        let mut validator_updates = state
            .get_validator_updates()
            .await
            .wrap_err("failed to read validator updates from storage")?;
        validator_updates.push_update(self.action.clone());
        state
            .put_validator_updates(validator_updates)
            .wrap_err("failed to write validator updates to storage")?;
        Ok(())
    }

    pub(super) fn action(&self) -> &ValidatorUpdate {
        &self.action
    }
}

impl AssetTransfer for CheckedValidatorUpdate {
    fn transfer_asset_and_amount(&self) -> Option<(IbcPrefixed, u128)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use astria_core::{
        crypto::VerificationKey,
        protocol::transaction::v1::action::*,
    };

    use super::*;
    use crate::{
        benchmark_and_test_utils::astria_address,
        checked_actions::CheckedSudoAddressChange,
        test_utils::{
            assert_error_contains,
            Fixture,
            ALICE,
            BOB,
            SUDO_ADDRESS_BYTES,
        },
    };

    #[cfg(test)]
    pub(super) fn dummy_validator_update(
        power: u32,
        verification_key_bytes: [u8; 32],
    ) -> ValidatorUpdate {
        ValidatorUpdate {
            power,
            verification_key: VerificationKey::try_from(verification_key_bytes).unwrap(),
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_sudo_address() {
        let fixture = Fixture::default_initialized().await;

        let tx_signer = [2_u8; ADDRESS_LEN];
        assert_ne!(*SUDO_ADDRESS_BYTES, tx_signer);

        let action = dummy_validator_update(100, [0; 32]);
        let err = fixture
            .new_checked_action(action, tx_signer)
            .await
            .unwrap_err();
        assert_error_contains(
            &err,
            "transaction signer not authorized to update validator set",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_removing_non_existent_validator() {
        let fixture = Fixture::default_initialized().await;

        let action = dummy_validator_update(0, [10; 32]);

        let err = fixture
            .new_checked_action(action, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap_err();
        assert_error_contains(&err, "cannot remove a non-existing validator");
    }

    #[tokio::test]
    async fn should_fail_construction_if_removing_only_validator() {
        let mut fixture = Fixture::uninitialized().await;
        fixture
            .chain_initializer()
            .with_genesis_validators(Some((ALICE.verification_key(), 100)))
            .init()
            .await;

        let action = dummy_validator_update(0, ALICE.verification_key().to_bytes());

        let err = fixture
            .new_checked_action(action, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap_err();
        assert_error_contains(&err, "cannot remove the only validator");
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::default_initialized().await;

        // Construct the checked action while the sudo address is still the tx signer so
        // construction succeeds.
        let action = dummy_validator_update(99, ALICE.verification_key().to_bytes());
        let checked_action: CheckedValidatorUpdate = fixture
            .new_checked_action(action, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();

        // Change the sudo address to something other than the tx signer.
        let sudo_address_change = SudoAddressChange {
            new_address: astria_address(&[2; ADDRESS_LEN]),
        };
        let checked_sudo_address_change: CheckedSudoAddressChange = fixture
            .new_checked_action(sudo_address_change, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();
        checked_sudo_address_change
            .execute(fixture.state_mut())
            .await
            .unwrap();
        let new_sudo_address = fixture.state().get_sudo_address().await.unwrap();
        assert_ne!(*SUDO_ADDRESS_BYTES, new_sudo_address);

        // Try to execute the checked action now - should fail due to signer no longer being
        // authorized.
        let err = checked_action
            .execute(fixture.state_mut())
            .await
            .unwrap_err();
        assert_error_contains(
            &err,
            "transaction signer not authorized to update validator set",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_removing_non_existent_validator() {
        let mut fixture = Fixture::default_initialized().await;

        // Construct two checked actions to remove the same validator while it is still a validator
        // so construction succeeds.
        let action = dummy_validator_update(0, ALICE.verification_key().to_bytes());
        let checked_action_1: CheckedValidatorUpdate = fixture
            .new_checked_action(action.clone(), *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();
        let checked_action_2: CheckedValidatorUpdate = fixture
            .new_checked_action(action, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();

        // Execute the first checked validator update.  We need to also run `end_block` in the
        // `AuthorityComponent` to actually have the validator set updated.
        checked_action_1.execute(fixture.state_mut()).await.unwrap();
        fixture.authority_component_end_block().await;

        // Try to execute the second checked action now - should fail due to validator no longer
        // being in the set.
        let err = checked_action_2
            .execute(fixture.state_mut())
            .await
            .unwrap_err();
        assert_error_contains(&err, "cannot remove a non-existing validator");
    }

    #[tokio::test]
    async fn should_fail_execution_if_removing_only_validator() {
        let mut fixture = Fixture::uninitialized().await;

        // Construct two checked actions to remove the only two validators while they are still
        // validators so construction succeeds.
        fixture
            .chain_initializer()
            .with_genesis_validators([
                (ALICE.verification_key(), 100),
                (BOB.verification_key(), 100),
            ])
            .init()
            .await;

        let action_1 = dummy_validator_update(0, ALICE.verification_key().to_bytes());
        let checked_action_1: CheckedValidatorUpdate = fixture
            .new_checked_action(action_1, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();
        let action_2 = dummy_validator_update(0, BOB.verification_key().to_bytes());
        let checked_action_2: CheckedValidatorUpdate = fixture
            .new_checked_action(action_2, *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();

        // Execute the first checked validator update.  We need to also run `end_block` in the
        // `AuthorityComponent` to actually have the validator set updated.
        checked_action_1.execute(fixture.state_mut()).await.unwrap();
        fixture.authority_component_end_block().await;

        // Try to execute the second checked action now - should fail due to validator being the
        // only validator in the set.
        let err = checked_action_2
            .execute(fixture.state_mut())
            .await
            .unwrap_err();
        assert_error_contains(&err, "cannot remove the only validator");
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::default_initialized().await;

        let action = dummy_validator_update(99, ALICE.verification_key().to_bytes());

        let checked_action: CheckedValidatorUpdate = fixture
            .new_checked_action(action.clone(), *SUDO_ADDRESS_BYTES)
            .await
            .unwrap()
            .into();
        checked_action.execute(fixture.state_mut()).await.unwrap();

        let validator_updates = fixture.state().get_validator_updates().await.unwrap();
        assert_eq!(validator_updates.len(), 1);
        let retrieved_update = validator_updates.get(&action.verification_key).unwrap();
        assert_eq!(*retrieved_update, action);
    }
}
