mod checks;
pub(crate) mod query;
mod state_ext;

use std::fmt;

use anyhow::{
    ensure,
    Context as _,
};
use astria_core::protocol::transaction::v1alpha1::{
    action::Action,
    SignedTransaction,
};
pub(crate) use checks::{
    check_balance_for_total_fees_and_transfers,
    check_balance_mempool,
    check_chain_id_mempool,
    check_nonce_mempool,
};
use cnidarium::StateWrite;
// Conditional to quiet warnings. This object is used throughout the codebase,
// but is never explicitly named - hence Rust warns about it being unused.
// #[cfg(test)]
// pub(crate) use state_ext::TransactionContext;
pub(crate) use state_ext::{
    StateReadExt,
    StateWriteExt,
};

use crate::{
    accounts::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    app::ActionHandler,
    bridge::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    cache::Cache,
    ibc::{
        host_interface::AstriaHost,
        StateReadExt as _,
    },
    state_ext::StateReadExt as _,
};

#[derive(Debug)]
pub(crate) struct InvalidChainId(pub(crate) String);

impl fmt::Display for InvalidChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "provided chain id {} does not match expected chain id",
            self.0,
        )
    }
}

impl std::error::Error for InvalidChainId {}

#[derive(Debug)]
pub(crate) struct InvalidNonce(pub(crate) u32);

impl fmt::Display for InvalidNonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "provided nonce {} does not match expected next nonce",
            self.0,
        )
    }
}

impl std::error::Error for InvalidNonce {}

#[async_trait::async_trait]
impl ActionHandler for SignedTransaction {
    type CheckStatelessContext = ();

    async fn check_stateless(&self, _context: Self::CheckStatelessContext) -> anyhow::Result<()> {
        ensure!(!self.actions().is_empty(), "must have at least one action");

        for action in self.actions() {
            match action {
                Action::Transfer(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for TransferAction")?,
                Action::Sequence(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for SequenceAction")?,
                Action::ValidatorUpdate(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for ValidatorUpdateAction")?,
                Action::SudoAddressChange(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for SudoAddressChangeAction")?,
                Action::FeeChange(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for FeeChangeAction")?,
                Action::Ibc(act) => {
                    let action = act
                        .clone()
                        .with_handler::<crate::ibc::ics20_transfer::Ics20Transfer, AstriaHost>();
                    action
                        .check_stateless(())
                        .await
                        .context("stateless check failed for IbcAction")?;
                }
                Action::Ics20Withdrawal(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for Ics20WithdrawalAction")?,
                Action::IbcRelayerChange(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for IbcRelayerChangeAction")?,
                Action::FeeAssetChange(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for FeeAssetChangeAction")?,
                Action::InitBridgeAccount(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for InitBridgeAccountAction")?,
                Action::BridgeLock(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for BridgeLockAction")?,
                Action::BridgeUnlock(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for BridgeLockAction")?,
                Action::BridgeSudoChange(act) => act
                    .check_stateless(())
                    .await
                    .context("stateless check failed for BridgeSudoChangeAction")?,
            }
        }
        Ok(())
    }

    // allowed / FIXME: because most lines come from delegating (and error wrapping) to the
    // individual actions. This could be tidied up by implementing `ActionHandler for Action`
    // and letting it delegate.
    #[allow(clippy::too_many_lines)]
    async fn check_and_execute<S: StateWrite>(
        &self,
        mut state: S,
        cache: &Cache,
    ) -> anyhow::Result<()> {
        // Add the current signed transaction into the ephemeral state in case
        // downstream actions require access to it.
        // XXX: This must be deleted at the end of `check_stateful`.
        state.put_current_source(self);

        // Transactions must match the chain id of the node.
        let chain_id = state.get_chain_id().await?;
        ensure!(
            self.chain_id() == chain_id.as_str(),
            InvalidChainId(self.chain_id().to_string())
        );

        // Nonce should be equal to the number of executed transactions before this tx.
        // First tx has nonce 0.
        let curr_nonce = state
            .get_account_nonce(self.address_bytes(), cache)
            .await
            .context("failed to get nonce for transaction signer")?;
        ensure!(curr_nonce == self.nonce(), InvalidNonce(self.nonce()));

        // Should have enough balance to cover all actions.
        check_balance_for_total_fees_and_transfers(self, &state, cache)
            .await
            .context("failed to check balance for total fees and transfers")?;

        if state
            .get_bridge_account_rollup_id(self, cache)
            .await
            .context("failed to check account rollup id")?
            .is_some()
        {
            state.put_last_transaction_hash_for_bridge_account(
                self,
                &self.sha256_of_proto_encoding(),
            );
        }

        let from_nonce = state
            .get_account_nonce(self, cache)
            .await
            .context("failed getting nonce of transaction signer")?;
        let next_nonce = from_nonce
            .checked_add(1)
            .context("overflow occurred incrementing stored nonce")?;
        state
            .put_account_nonce(self, next_nonce)
            .context("failed updating `from` nonce")?;

        // FIXME: this should create one span per `check_and_execute`
        for action in self.actions() {
            match action {
                Action::Transfer(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("executing transfer action failed")?,
                Action::Sequence(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("executing sequence action failed")?,
                Action::ValidatorUpdate(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("executing validor update")?,
                Action::SudoAddressChange(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("executing sudo address change failed")?,
                Action::FeeChange(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("executing fee change failed")?,
                Action::Ibc(act) => {
                    // FIXME: this check should be moved to check_and_execute, as it now has
                    // access to the the signer through state. However, what's the correct
                    // ibc AppHandler call to do it? Can we just update one of the trait methods
                    // of crate::ibc::ics20_transfer::Ics20Transfer?
                    ensure!(
                        state
                            .is_ibc_relayer(self)
                            .await
                            .context("failed to check if address is IBC relayer")?,
                        "only IBC sudo address can execute IBC actions"
                    );
                    let action = act
                        .clone()
                        .with_handler::<crate::ibc::ics20_transfer::Ics20Transfer, AstriaHost>();
                    action
                        .check_and_execute(&mut state)
                        .await
                        .context("failed executing ibc action")?;
                }
                Action::Ics20Withdrawal(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing ics20 withdrawal")?,
                Action::IbcRelayerChange(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing ibc relayer change")?,
                Action::FeeAssetChange(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing fee asseet change")?,
                Action::InitBridgeAccount(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing init bridge account")?,
                Action::BridgeLock(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing bridge lock")?,
                Action::BridgeUnlock(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing bridge unlock")?,
                Action::BridgeSudoChange(act) => act
                    .check_and_execute(&mut state, cache)
                    .await
                    .context("failed executing bridge sudo change")?,
            }
        }

        // XXX: Delete the current transaction data from the ephemeral state.
        state.delete_current_source();
        Ok(())
    }
}
