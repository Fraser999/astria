use anyhow::{
    Context,
    Result,
};
use astria_core::{
    primitive::v1::Address,
    protocol::transaction::v1alpha1::action::ValidatorUpdate,
};
use tendermint::abci::request::{
    BeginBlock,
    EndBlock,
};
use tracing::instrument;

use super::{
    StateReadExt,
    StateWriteExt,
    ValidatorSet,
};
use crate::{
    component::Component,
    storage::DeltaDelta,
};

#[derive(Default)]
pub(crate) struct AuthorityComponent;

#[derive(Debug)]
pub(crate) struct AuthorityComponentAppState {
    pub(crate) authority_sudo_address: Address,
    pub(crate) genesis_validators: Vec<ValidatorUpdate>,
}

#[async_trait::async_trait]
impl Component for AuthorityComponent {
    type AppState = AuthorityComponentAppState;

    #[instrument(name = "AuthorityComponent::init_chain", skip_all)]
    async fn init_chain(state: &DeltaDelta, app_state: &Self::AppState) -> Result<()> {
        // set sudo key and initial validator set
        state.put_sudo_address(app_state.authority_sudo_address);
        let genesis_validators = app_state.genesis_validators.clone();
        state.put_validator_set(ValidatorSet::new_from_updates(genesis_validators));
        Ok(())
    }

    #[instrument(name = "AuthorityComponent::begin_block", skip_all)]
    async fn begin_block(state: &DeltaDelta, begin_block: &BeginBlock) -> Result<()> {
        let mut current_set = state
            .get_validator_set()
            .await
            .context("failed getting validator set")?;

        for misbehaviour in &begin_block.byzantine_validators {
            current_set.remove(misbehaviour.validator.address);
        }

        state.put_validator_set(current_set);
        Ok(())
    }

    #[instrument(name = "AuthorityComponent::end_block", skip_all)]
    async fn end_block(state: &DeltaDelta, _end_block: &EndBlock) -> Result<()> {
        // update validator set
        let validator_updates = state
            .get_validator_updates()
            .await
            .context("failed getting validator updates")?;

        let mut current_set = state
            .get_validator_set()
            .await
            .context("failed getting validator set")?;
        current_set.apply_updates(validator_updates);

        state.put_validator_set(current_set);
        Ok(())
    }
}
