use std::collections::HashMap;

use astria_core::{
    crypto::{
        Signature,
        VerificationKey,
        ADDRESS_LENGTH,
    },
    generated::astria::protocol::transaction::v1 as raw,
    primitive::v1::{
        asset::IbcPrefixed,
        RollupId,
        TransactionId,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::{
        Action,
        Group,
        Transaction,
        TransactionParams,
    },
    Protobuf,
};
use bytes::Bytes;
use cnidarium::{
    StateRead,
    StateWrite,
};
use futures::future::try_join_all;
use prost::Message as _;
use sha2::Digest as _;
use tracing::{
    instrument,
    Level,
};

pub(crate) use self::error::CheckedTransactionError;
use crate::{
    accounts::{
        AddressBytes,
        StateReadExt as _,
        StateWriteExt as _,
    },
    app::StateReadExt as _,
    bridge::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    checked_actions::{
        utils::total_fees,
        ActionRef,
        CheckedAction,
    },
};

mod error;
#[cfg(test)]
mod tests;

const MAX_TX_BYTES: usize = 256_000;

#[derive(Debug)]
pub(crate) struct CheckedTransaction {
    tx_id: TransactionId,
    actions: Vec<CheckedAction>,
    group: Group,
    params: TransactionParams,
    body_bytes: Bytes,
    verification_key: VerificationKey,
    signature: Signature,
    tx_bytes: Bytes,
}

impl CheckedTransaction {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(crate) async fn new<S: StateRead>(
        tx_bytes: Bytes,
        state: &S,
    ) -> Result<Self, CheckedTransactionError> {
        let tx_len = tx_bytes.len();
        if tx_len > MAX_TX_BYTES {
            return Err(CheckedTransactionError::TooLarge {
                tx_len,
            });
        }

        let raw_tx =
            raw::Transaction::decode(tx_bytes.clone()).map_err(CheckedTransactionError::Decode)?;
        let tx = Transaction::try_from_raw(raw_tx).map_err(CheckedTransactionError::Convert)?;

        if tx.actions().is_empty() {
            return Err(CheckedTransactionError::NoActions);
        }

        let tx_id = TransactionId::new(sha2::Sha256::digest(&tx_bytes).into());
        let tx_chain_id = tx.chain_id().to_string();

        let (unchecked_actions, group, params, body_bytes, verification_key, signature) =
            tx.into_parts();
        let tx_signer = *verification_key.address_bytes();
        let checked_actions =
            match convert_actions(unchecked_actions, tx_signer, tx_id, state).await {
                Ok(checked_actions) => checked_actions,
                Err(error) => {
                    return Err(error);
                }
            };

        let chain_id = state.get_chain_id().await.map_err(|source| {
            CheckedTransactionError::internal("failed to get chain id from storage", source)
        })?;
        if tx_chain_id != chain_id.as_str() {
            return Err(CheckedTransactionError::ChainIdMismatch {
                expected: chain_id.as_str().to_string(),
                tx_chain_id,
            });
        }

        Ok(Self {
            tx_id,
            actions: checked_actions,
            group,
            params,
            body_bytes,
            verification_key,
            signature,
            tx_bytes,
        })
    }

    pub(crate) fn id(&self) -> &TransactionId {
        &self.tx_id
    }

    #[must_use]
    pub(crate) fn checked_actions(&self) -> &[CheckedAction] {
        &self.actions
    }

    pub(crate) fn group(&self) -> Group {
        self.group
    }

    pub(crate) fn nonce(&self) -> u32 {
        self.params.nonce()
    }

    pub(crate) fn chain_id(&self) -> &str {
        self.params.chain_id()
    }

    pub(crate) fn verification_key(&self) -> &VerificationKey {
        &self.verification_key
    }

    /// Returns the bytes of the encoded `Transaction` from which this `CheckedTransaction` is
    /// constructed.
    pub(crate) fn encoded_bytes(&self) -> &Bytes {
        &self.tx_bytes
    }

    /// Returns an iterator over the rollup ID and data bytes of all `RollupDataSubmission`s in this
    /// transaction's actions, in the order in which they occur in the transaction.
    pub(crate) fn rollup_data_bytes(&self) -> impl Iterator<Item = (&RollupId, &Bytes)> {
        self.actions.iter().filter_map(|checked_action| {
            if let CheckedAction::RollupDataSubmission(rollup_submission) = checked_action {
                Some((
                    &rollup_submission.action().rollup_id,
                    &rollup_submission.action().data,
                ))
            } else {
                None
            }
        })
    }

    pub(crate) async fn total_costs<S: StateRead>(
        &self,
        state: &S,
    ) -> Result<HashMap<IbcPrefixed, u128>, CheckedTransactionError> {
        let mut cost_by_asset = total_fees(self.actions.iter().map(ActionRef::from), state).await?;

        for action in &self.actions {
            if let Some((asset, amount)) = action.asset_and_amount_to_transfer() {
                cost_by_asset
                    .entry(asset)
                    .and_modify(|amt| *amt = amt.saturating_add(amount))
                    .or_insert(amount);
            }
        }

        Ok(cost_by_asset)
    }

    pub(crate) async fn run_mutable_checks<S: StateRead>(
        &self,
        state: S,
    ) -> Result<(), CheckedTransactionError> {
        for action in &self.actions {
            action.run_mutable_checks(&state).await?;
        }
        Ok(())
    }

    pub(super) async fn execute<S: StateWrite>(
        &self,
        mut state: S,
    ) -> Result<(), CheckedTransactionError> {
        // Nonce should be equal to the number of executed transactions before this tx.
        // First tx has nonce 0.
        let current_nonce = state
            .get_account_nonce(self.address_bytes())
            .await
            .map_err(|source| {
                CheckedTransactionError::internal("failed to read nonce from storage", source)
            })?;
        let tx_nonce = self.params.nonce();
        if current_nonce != tx_nonce {
            return Err(CheckedTransactionError::InvalidNonce {
                expected: current_nonce,
                tx_nonce,
            });
        };

        if state
            .get_bridge_account_rollup_id(self)
            .await
            .map_err(|source| {
                CheckedTransactionError::internal(
                    "failed to read bridge account rollup id from storage",
                    source,
                )
            })?
            .is_some()
        {
            state
                .put_last_transaction_id_for_bridge_account(self, self.tx_id)
                .map_err(|source| {
                    CheckedTransactionError::internal(
                        "failed to write last transaction id to storage",
                        source,
                    )
                })?;
        }

        let next_nonce = current_nonce
            .checked_add(1)
            .ok_or(CheckedTransactionError::NonceOverflowed)?;
        state
            .put_account_nonce(self, next_nonce)
            .map_err(|source| CheckedTransactionError::internal("failed updating nonce", source))?;

        // FIXME: this should create one span per `check_and_execute`
        let tx_signer = *self.verification_key.address_bytes();
        for (index, action) in self.actions.iter().enumerate() {
            let index =
                u64::try_from(index).map_err(|_| CheckedTransactionError::ActionIndexOverflowed)?;
            action
                .execute_and_pay_fees(&mut state, &tx_signer, &self.tx_id, index)
                .await?;
        }
        Ok(())
    }
}

impl AddressBytes for CheckedTransaction {
    fn address_bytes(&self) -> &[u8; ADDRESS_LEN] {
        self.verification_key.address_bytes()
    }
}

async fn convert_actions<S: StateRead>(
    unchecked_actions: Vec<Action>,
    tx_signer: [u8; ADDRESS_LENGTH],
    tx_id: TransactionId,
    state: &S,
) -> Result<Vec<CheckedAction>, CheckedTransactionError> {
    let actions_futures =
        unchecked_actions
            .into_iter()
            .enumerate()
            .map(|(index, unchecked_action)| async move {
                match unchecked_action {
                    Action::RollupDataSubmission(action) => {
                        CheckedAction::new_rollup_data_submission(action)
                    }
                    Action::Transfer(action) => {
                        CheckedAction::new_transfer(action, tx_signer, state).await
                    }
                    Action::ValidatorUpdate(action) => {
                        CheckedAction::new_validator_update(action, tx_signer, state).await
                    }
                    Action::SudoAddressChange(action) => {
                        CheckedAction::new_sudo_address_change(action, tx_signer, state).await
                    }
                    Action::Ibc(action) => {
                        CheckedAction::new_ibc_relay(action, tx_signer, state).await
                    }
                    Action::IbcSudoChange(action) => {
                        CheckedAction::new_ibc_sudo_change(action, tx_signer, state).await
                    }
                    Action::Ics20Withdrawal(action) => {
                        CheckedAction::new_ics20_withdrawal(action, tx_signer, state).await
                    }
                    Action::IbcRelayerChange(action) => {
                        CheckedAction::new_ibc_relayer_change(action, tx_signer, state).await
                    }
                    Action::FeeAssetChange(action) => {
                        CheckedAction::new_fee_asset_change(action, tx_signer, state).await
                    }
                    Action::InitBridgeAccount(action) => {
                        CheckedAction::new_init_bridge_account(action, tx_signer, state).await
                    }
                    Action::BridgeLock(action) => {
                        let position_in_tx = u64::try_from(index)
                            .expect("there should be less than `u64::MAX` actions in tx");
                        CheckedAction::new_bridge_lock(
                            action,
                            tx_signer,
                            tx_id,
                            position_in_tx,
                            state,
                        )
                        .await
                    }
                    Action::BridgeUnlock(action) => {
                        CheckedAction::new_bridge_unlock(action, tx_signer, state).await
                    }
                    Action::BridgeSudoChange(action) => {
                        CheckedAction::new_bridge_sudo_change(action, tx_signer, state).await
                    }
                    Action::BridgeTransfer(action) => {
                        let position_in_tx = u64::try_from(index)
                            .expect("there should be less than `u64::MAX` actions in tx");
                        CheckedAction::new_bridge_transfer(
                            action,
                            tx_signer,
                            tx_id,
                            position_in_tx,
                            state,
                        )
                        .await
                    }
                    Action::FeeChange(action) => {
                        CheckedAction::new_fee_change(action, tx_signer, state).await
                    }
                    Action::RecoverIbcClient(action) => {
                        CheckedAction::new_recover_ibc_client(action, tx_signer, state).await
                    }
                    Action::CurrencyPairsChange(action) => {
                        CheckedAction::new_currency_pairs_change(action, tx_signer, state).await
                    }
                    Action::MarketsChange(action) => {
                        CheckedAction::new_markets_change(action, tx_signer, state).await
                    }
                }
            });

    try_join_all(actions_futures)
        .await
        .map_err(CheckedTransactionError::CheckedAction)
}
