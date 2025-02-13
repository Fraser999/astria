use astria_core::{
    primitive::v1::{
        asset::Denom,
        TransactionId,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::BridgeLock,
    sequencerblock::v1::block::Deposit,
};
use astria_eyre::eyre::{
    bail,
    ensure,
    OptionExt as _,
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
    accounts::StateWriteExt as _,
    address::StateReadExt as _,
    assets::StateReadExt as _,
    bridge::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    utils::create_deposit_event,
};

pub(crate) type CheckedBridgeLock = CheckedBridgeLockImpl<true>;

/// This struct provides the implementation details of the checked bridge lock action.
///
/// It is also used to perform checks on a bridge transfer action, which is essentially a cross
/// between a bridge unlock and a bridge lock.
///
/// A `BridgeLock` action uses the tx signer as the source account, and does not allow the tx signer
/// to be a bridge account, whereas `BridgeTransfer` provides a `bridge_account` as the source where
/// this may not be tx signer's account, and is required to be a bridge account. Hence a bridge lock
/// is implemented via `CheckedBridgeLockImpl<true>` and has methods to allow checking AND executing
/// the action, while the checks relevant to a bridge transfer are implemented via
/// `CheckedBridgeLockImpl<false>`, where this has no method supporting execution.
#[derive(Debug)]
pub(crate) struct CheckedBridgeLockImpl<const PURE_LOCK: bool> {
    action: BridgeLock,
    tx_signer: TransactionSignerAddressBytes,
    tx_id: TransactionId,
    /// The index of this action in the transaction.
    position_in_tx: u64,
    /// The deposit created from this action.
    deposit: Deposit,
}

impl<const PURE_LOCK: bool> CheckedBridgeLockImpl<PURE_LOCK> {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: BridgeLock,
        tx_signer: [u8; ADDRESS_LEN],
        tx_id: TransactionId,
        position_in_tx: u64,
        state: S,
    ) -> Result<Self> {
        state
            .ensure_base_prefix(&action.to)
            .await
            .wrap_err("destination address has an unsupported prefix")?;

        // check that the asset to be transferred matches the bridge account asset.
        // this also implicitly ensures the recipient is a bridge account.
        let allowed_asset = state
            .get_bridge_account_ibc_asset(&action.to)
            .await
            .wrap_err("failed to get bridge account asset ID; account is not a bridge account")?;
        ensure!(
            allowed_asset == action.asset.to_ibc_prefixed(),
            "asset ID is not authorized for transfer to bridge account",
        );

        // Try to construct the `Deposit`.
        let rollup_id = state
            .get_bridge_account_rollup_id(&action.to)
            .await
            .wrap_err("failed to get bridge account rollup id")?
            .ok_or_eyre("bridge lock must be sent to a bridge account")?;
        // Map asset to trace prefixed asset for deposit, if it is not already. The IBC asset cannot
        // be changed once set in state, so if `map_ibc_to_trace_prefixed_asset` succeeds now it
        // can't fail later during execution.
        let deposit_asset = match &action.asset {
            Denom::TracePrefixed(asset) => asset.clone(),
            Denom::IbcPrefixed(asset) => state
                .map_ibc_to_trace_prefixed_asset(asset)
                .await
                .wrap_err("failed to map IBC asset to trace prefixed asset")?
                .ok_or_eyre("mapping from IBC prefixed bridge asset to trace prefixed not found")?,
        };
        let deposit = Deposit {
            bridge_address: action.to,
            rollup_id,
            amount: action.amount,
            asset: deposit_asset.into(),
            destination_chain_address: action.destination_chain_address.clone(),
            source_transaction_id: tx_id,
            source_action_index: position_in_tx,
        };

        let checked_action = Self {
            action,
            tx_signer: tx_signer.into(),
            tx_id,
            position_in_tx,
            deposit,
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    pub(super) async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        if PURE_LOCK
            && state
                .is_a_bridge_account(&self.tx_signer)
                .await
                .wrap_err("failed to check if signer is a bridge account")?
        {
            bail!("bridge accounts cannot send bridge locks");
        }

        Ok(())
    }

    pub(super) fn record_deposit<S: StateWrite>(&self, mut state: S) {
        let deposit_abci_event = create_deposit_event(&self.deposit);
        state.cache_deposit_event(self.deposit.clone());
        state.record(deposit_abci_event);
    }
}

impl CheckedBridgeLockImpl<true> {
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

        self.record_deposit(state);

        Ok(())
    }
}

impl CheckedBridgeLockImpl<false> {
    pub(super) fn action(&self) -> &BridgeLock {
        &self.action
    }

    pub(super) fn tx_id(&self) -> &TransactionId {
        &self.tx_id
    }

    pub(super) fn position_in_tx(&self) -> u64 {
        self.position_in_tx
    }
}

// NOTE: unit tests here cover only `CheckedBridgeLockImpl<true>`.  Test coverage of
// `CheckedBridgeLockImpl<false>` is in `checked_actions::bridge_transfer`.
#[cfg(test)]
mod tests {
    use astria_core::{
        primitive::v1::{
            asset::{
                IbcPrefixed,
                TracePrefixed,
            },
            RollupId,
        },
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
        assets::StateWriteExt as _,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            nria,
            ASTRIA_PREFIX,
        },
    };

    fn new_bridge_lock() -> BridgeLock {
        BridgeLock {
            to: astria_address(&[50; ADDRESS_LEN]),
            amount: 100,
            asset: nria().into(),
            fee_asset: nria().into(),
            destination_chain_address: "test-chain".to_string(),
        }
    }

    async fn new_checked_bridge_lock(
        fixture: &Fixture,
        action: BridgeLock,
    ) -> Result<CheckedBridgeLock> {
        CheckedBridgeLock::new(
            action,
            fixture.tx_signer,
            TransactionId::new([10; 32]),
            11,
            &fixture.state,
        )
        .await
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeLock {
            to: address_with_prefix([50; ADDRESS_LEN], prefix),
            ..new_bridge_lock()
        };
        let err = new_checked_bridge_lock(&fixture, action).await.unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_asset_not_allowed() {
        let mut fixture = Fixture::new().await;

        let action = BridgeLock {
            asset: Denom::IbcPrefixed(IbcPrefixed::new([10; 32])),
            ..new_bridge_lock()
        };
        fixture
            .state
            .put_bridge_account_ibc_asset(&action.to, nria())
            .unwrap();

        let err = new_checked_bridge_lock(&fixture, action).await.unwrap_err();

        assert_eyre_error(
            &err,
            "asset ID is not authorized for transfer to bridge account",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_account_rollup_id_not_found() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_lock();
        fixture
            .bridge_initializer(action.to)
            .with_no_rollup_id()
            .init();

        let err = new_checked_bridge_lock(&fixture, action).await.unwrap_err();

        assert_eyre_error(&err, "bridge lock must be sent to a bridge account");
    }

    #[tokio::test]
    async fn should_fail_construction_if_asset_mapping_fails() {
        let mut fixture = Fixture::new().await;

        let asset = Denom::IbcPrefixed(IbcPrefixed::new([10; 32]));
        let action = BridgeLock {
            asset: asset.clone(),
            ..new_bridge_lock()
        };
        fixture
            .bridge_initializer(action.to)
            .with_asset(asset)
            .init();

        let err = new_checked_bridge_lock(&fixture, action).await.unwrap_err();

        assert_eyre_error(
            &err,
            "mapping from IBC prefixed bridge asset to trace prefixed not found",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_bridge_account() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_lock();
        fixture
            .state
            .put_bridge_account_ibc_asset(&action.to, nria())
            .unwrap();
        fixture
            .state
            .put_bridge_account_rollup_id(&action.to, RollupId::new([1; 32]))
            .unwrap();
        fixture
            .state
            .put_bridge_account_rollup_id(&fixture.tx_signer, RollupId::new([2; 32]))
            .unwrap();

        let err = new_checked_bridge_lock(&fixture, action).await.unwrap_err();

        assert_eyre_error(&err, "bridge accounts cannot send bridge locks");
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_bridge_account() {
        let mut fixture = Fixture::new().await;

        // Construct a checked bridge lock while the signer account is not a bridge account.
        let action = new_bridge_lock();
        fixture
            .state
            .put_bridge_account_ibc_asset(&action.to, nria())
            .unwrap();
        fixture
            .state
            .put_bridge_account_rollup_id(&action.to, RollupId::new([1; 32]))
            .unwrap();
        let checked_action = new_checked_bridge_lock(&fixture, action).await.unwrap();

        // Initialize the signer's account as a bridge account.
        let init_bridge_account = InitBridgeAccount {
            rollup_id: RollupId::new([2; 32]),
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

        // Try to execute the checked bridge lock now - should fail due to bridge account now
        // existing.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "bridge accounts cannot send bridge locks");
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_account_has_insufficient_balance() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_lock();
        fixture
            .state
            .put_bridge_account_ibc_asset(&action.to, nria())
            .unwrap();
        fixture
            .state
            .put_bridge_account_rollup_id(&action.to, RollupId::new([1; 32]))
            .unwrap();
        let checked_action = new_checked_bridge_lock(&fixture, action).await.unwrap();

        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "failed to decrease signer account balance");
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;

        // Construct the checked bridge lock while the account has insufficient balance to ensure
        // balance checks are only part of execution.
        let action = new_bridge_lock();
        let rollup_id = RollupId::new([1; 32]);
        fixture
            .state
            .put_bridge_account_ibc_asset(&action.to, nria())
            .unwrap();
        fixture
            .state
            .put_bridge_account_rollup_id(&action.to, rollup_id)
            .unwrap();
        let checked_action = new_checked_bridge_lock(&fixture, action.clone())
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

        checked_action.execute(&mut fixture.state).await.unwrap();

        // Check the balances are correct after execution.
        assert_eq!(fixture.get_nria_balance(&fixture.tx_signer).await, 0);
        assert_eq!(fixture.get_nria_balance(&action.to).await, action.amount);

        // Check the deposit is recorded.
        let deposits = fixture
            .state
            .get_cached_block_deposits()
            .get(&rollup_id)
            .unwrap()
            .clone();
        assert_eq!(deposits.len(), 1);
        let deposit = &deposits[0];
        assert_eq!(deposit.bridge_address, action.to);
        assert_eq!(deposit.rollup_id, rollup_id);
        assert_eq!(deposit.amount, action.amount);
        assert_eq!(deposit.asset, action.asset);
        assert_eq!(
            deposit.destination_chain_address,
            action.destination_chain_address
        );
        assert_eq!(deposit.source_transaction_id, checked_action.tx_id);
        assert_eq!(deposit.source_action_index, checked_action.position_in_tx);

        // Check the deposit event is cached.
        let deposit_events = fixture.state.flatten().1.take_events();
        assert_eq!(deposit_events.len(), 1);
        assert_eq!(deposit_events[0].kind, "tx.deposit");
    }

    #[tokio::test]
    async fn should_map_ibc_to_trace_prefixed_for_deposit() {
        let mut fixture = Fixture::new().await;

        // Construct the bridge lock with an IBC denom, and check it is recorded in the `Deposit` as
        // trace-prefixed.
        let trace_asset = "trace_asset".parse::<TracePrefixed>().unwrap();
        let ibc_asset = trace_asset.to_ibc_prefixed();
        let action = BridgeLock {
            asset: Denom::IbcPrefixed(ibc_asset),
            ..new_bridge_lock()
        };

        fixture
            .state
            .put_bridge_account_ibc_asset(&action.to, ibc_asset)
            .unwrap();
        let rollup_id = RollupId::new([1; 32]);
        fixture
            .state
            .put_bridge_account_rollup_id(&action.to, rollup_id)
            .unwrap();
        fixture.state.put_ibc_asset(trace_asset.clone()).unwrap();
        let checked_action = new_checked_bridge_lock(&fixture, action.clone())
            .await
            .unwrap();

        fixture
            .state
            .increase_balance(&fixture.tx_signer, &action.asset, action.amount)
            .await
            .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        let deposits = &fixture
            .state
            .get_cached_block_deposits()
            .get(&rollup_id)
            .unwrap()
            .clone();
        assert!(deposits[0].asset.as_trace_prefixed().is_some());
    }
}
