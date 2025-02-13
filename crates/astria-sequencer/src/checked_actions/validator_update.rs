use astria_core::{
    primitive::v1::ADDRESS_LEN,
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

use super::TransactionSignerAddressBytes;
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

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
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
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astria_core::{
        crypto::VerificationKey,
        protocol::transaction::v1::action::*,
    };

    use super::{
        super::{
            test_utils::Fixture,
            CheckedAction,
        },
        *,
    };
    use crate::{
        authority::{
            component::AuthorityComponent,
            ValidatorSet,
        },
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
        },
        component::Component as _,
    };

    fn new_validator_update(power: u32, verification_key_bytes: [u8; 32]) -> ValidatorUpdate {
        ValidatorUpdate {
            power,
            verification_key: VerificationKey::try_from(verification_key_bytes).unwrap(),
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Store a sudo address different from the tx signer address.
        let sudo_address = [2; ADDRESS_LEN];
        assert_ne!(fixture.tx_signer, sudo_address);
        fixture.state.put_sudo_address(sudo_address).unwrap();

        let action = new_validator_update(100, [0; 32]);
        let err = CheckedValidatorUpdate::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to update validator set",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_removing_non_existent_validator() {
        let mut fixture = Fixture::new().await;

        fixture
            .state
            .put_validator_set(ValidatorSet::new_from_updates(vec![new_validator_update(
                100, [9; 32],
            )]))
            .unwrap();

        let action = new_validator_update(0, [10; 32]);

        let err = CheckedValidatorUpdate::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "cannot remove a non-existing validator");
    }

    #[tokio::test]
    async fn should_fail_construction_if_removing_only_validator() {
        let mut fixture = Fixture::new().await;

        let verification_key_bytes = [10; 32];
        fixture
            .state
            .put_validator_set(ValidatorSet::new_from_updates(vec![new_validator_update(
                100,
                verification_key_bytes,
            )]))
            .unwrap();

        let action = new_validator_update(0, verification_key_bytes);

        let err = CheckedValidatorUpdate::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "cannot remove the only validator");
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_sudo_address() {
        let mut fixture = Fixture::new().await;

        // Construct the checked action while the sudo address is still the tx signer so
        // construction succeeds.
        let verification_key_bytes = [10; 32];
        fixture
            .state
            .put_validator_set(ValidatorSet::new_from_updates(vec![new_validator_update(
                100,
                verification_key_bytes,
            )]))
            .unwrap();
        let action = new_validator_update(99, verification_key_bytes);
        let checked_action = CheckedValidatorUpdate::new(action, fixture.tx_signer, &fixture.state)
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
            "transaction signer not authorized to update validator set",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_removing_non_existent_validator() {
        let mut fixture = Fixture::new().await;

        // Construct two checked actions to remove the same validator while it is still a validator
        // so construction succeeds.
        let verification_key_bytes = [10; 32];
        let validator_1 = new_validator_update(100, verification_key_bytes);
        let validator_2 = new_validator_update(100, [11; 32]);
        fixture
            .state
            .put_validator_set(ValidatorSet::new_from_updates(vec![
                validator_1,
                validator_2,
            ]))
            .unwrap();
        let action = new_validator_update(0, verification_key_bytes);
        let checked_action_1 =
            CheckedValidatorUpdate::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let checked_action_2 =
            CheckedValidatorUpdate::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first checked validator update.  We need to also run `end_block` to actually
        // have the validator set updated.
        checked_action_1.execute(&mut fixture.state).await.unwrap();
        let end_block_request = tendermint::abci::request::EndBlock {
            height: 1,
        };
        // Temporarily take ownership of the fixture's state member to allow passing an `Arc` to
        // `end_block`.
        let forked_state = fixture.state.fork();
        let state = std::mem::replace(&mut fixture.state, forked_state);
        let mut arc_state = Arc::new(state);
        AuthorityComponent::end_block(&mut arc_state, &end_block_request)
            .await
            .unwrap();
        // Give ownership of state back to `fixture`.
        let _ = std::mem::replace(&mut fixture.state, Arc::into_inner(arc_state).unwrap());

        // Try to execute the second checked action now - should fail due to validator no longer
        // being in the set.
        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "cannot remove a non-existing validator");
    }

    #[tokio::test]
    async fn should_fail_execution_if_removing_only_validator() {
        let mut fixture = Fixture::new().await;

        // Construct two checked actions to remove the only two validators while they are still
        // validators so construction succeeds.
        let verification_key_bytes_1 = [10; 32];
        let verification_key_bytes_2 = [11; 32];
        let validator_1 = new_validator_update(100, verification_key_bytes_1);
        let validator_2 = new_validator_update(100, verification_key_bytes_2);
        fixture
            .state
            .put_validator_set(ValidatorSet::new_from_updates(vec![
                validator_1,
                validator_2,
            ]))
            .unwrap();
        let action_1 = new_validator_update(0, verification_key_bytes_1);
        let checked_action_1 =
            CheckedValidatorUpdate::new(action_1, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let action_2 = new_validator_update(0, verification_key_bytes_2);
        let checked_action_2 =
            CheckedValidatorUpdate::new(action_2, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first checked validator update.  We need to also run `end_block` to actually
        // have the validator set updated.
        checked_action_1.execute(&mut fixture.state).await.unwrap();
        let end_block_request = tendermint::abci::request::EndBlock {
            height: 1,
        };
        // Temporarily take ownership of the fixture's state member to allow passing an `Arc` to
        // `end_block`.
        let forked_state = fixture.state.fork();
        let state = std::mem::replace(&mut fixture.state, forked_state);
        let mut arc_state = Arc::new(state);
        AuthorityComponent::end_block(&mut arc_state, &end_block_request)
            .await
            .unwrap();
        // Give ownership of state back to `fixture`.
        let _ = std::mem::replace(&mut fixture.state, Arc::into_inner(arc_state).unwrap());

        // Try to execute the second checked action now - should fail due to validator being the
        // only validator in the set.
        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "cannot remove the only validator");
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;

        let verification_key_bytes = [10; 32];
        fixture
            .state
            .put_validator_set(ValidatorSet::new_from_updates(vec![new_validator_update(
                100,
                verification_key_bytes,
            )]))
            .unwrap();

        let action = new_validator_update(99, verification_key_bytes);

        let checked_action =
            CheckedValidatorUpdate::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_action.execute(&mut fixture.state).await.unwrap();

        let validator_updates = fixture.state.get_validator_updates().await.unwrap();
        assert_eq!(validator_updates.len(), 1);
        let retrieved_update = validator_updates.get(&action.verification_key).unwrap();
        assert_eq!(*retrieved_update, action);
    }
}
