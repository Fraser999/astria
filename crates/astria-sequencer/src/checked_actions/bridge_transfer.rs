use astria_core::{
    primitive::v1::{
        TransactionId,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::{
        BridgeLock,
        BridgeTransfer,
        BridgeUnlock,
    },
};
use astria_eyre::eyre::{
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
    bridge_lock::CheckedBridgeLockImpl,
    bridge_unlock::CheckedBridgeUnlockImpl,
};
use crate::accounts::StateWriteExt as _;

#[derive(Debug)]
pub(crate) struct CheckedBridgeTransfer {
    checked_bridge_unlock: CheckedBridgeUnlockImpl<false>,
    checked_bridge_lock: CheckedBridgeLockImpl<false>,
}

impl CheckedBridgeTransfer {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        BridgeTransfer {
            to,
            amount,
            fee_asset,
            destination_chain_address,
            bridge_address,
            rollup_block_number,
            rollup_withdrawal_event_id,
        }: BridgeTransfer,
        tx_signer: [u8; ADDRESS_LEN],
        tx_id: TransactionId,
        position_in_tx: u64,
        state: S,
    ) -> Result<Self> {
        let bridge_unlock = BridgeUnlock {
            to,
            amount,
            memo: String::new(),
            rollup_withdrawal_event_id,
            rollup_block_number,
            fee_asset: fee_asset.clone(),
            bridge_address,
        };
        let checked_bridge_unlock =
            CheckedBridgeUnlockImpl::<false>::new(bridge_unlock, tx_signer, &state)
                .await
                .wrap_err("failed to construct checked bridge unlock for bridge transfer")?;

        let bridge_lock = BridgeLock {
            to,
            amount,
            asset: checked_bridge_unlock.bridge_account_ibc_asset().into(),
            fee_asset,
            destination_chain_address,
        };
        let checked_bridge_lock = CheckedBridgeLockImpl::<false>::new(
            bridge_lock,
            tx_signer,
            tx_id,
            position_in_tx,
            &state,
        )
        .await
        .wrap_err("failed to construct checked bridge lock for bridge transfer")?;

        let checked_action = Self {
            checked_bridge_unlock,
            checked_bridge_lock,
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;

        let from = &self.checked_bridge_unlock.action().bridge_address;
        let to = &self.checked_bridge_unlock.action().to;
        let asset = self.checked_bridge_unlock.bridge_account_ibc_asset();
        let amount = self.checked_bridge_unlock.action().amount;

        state
            .decrease_balance(from, asset, amount)
            .await
            .wrap_err("failed to decrease bridge account balance")?;
        state
            .increase_balance(to, asset, amount)
            .await
            .wrap_err("failed to increase destination account balance")?;

        self.checked_bridge_lock.record_deposit(&mut state);
        self.checked_bridge_unlock.record_withdrawal_event(state)
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        self.checked_bridge_unlock
            .run_mutable_checks(&state)
            .await
            .wrap_err("mutable checks for bridge unlock failed for bridge transfer")?;
        self.checked_bridge_lock
            .run_mutable_checks(&state)
            .await
            .wrap_err("mutable checks for bridge lock failed for bridge transfer")
    }
}

#[cfg(test)]
mod tests {
    use astria_core::{
        primitive::v1::{
            asset::{
                Denom,
                IbcPrefixed,
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
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            nria,
            ASTRIA_PREFIX,
        },
        bridge::{
            StateReadExt as _,
            StateWriteExt as _,
        },
    };

    fn new_bridge_transfer() -> BridgeTransfer {
        BridgeTransfer {
            to: astria_address(&[2; ADDRESS_LEN]),
            amount: 100,
            fee_asset: nria().into(),
            destination_chain_address: "test-chain".to_string(),
            bridge_address: astria_address(&[50; ADDRESS_LEN]),
            rollup_block_number: 10,
            rollup_withdrawal_event_id: "a-rollup-defined-hash".to_string(),
        }
    }

    async fn new_checked_bridge_transfer(
        fixture: &Fixture,
        action: BridgeTransfer,
    ) -> Result<CheckedBridgeTransfer> {
        CheckedBridgeTransfer::new(
            action,
            fixture.tx_signer,
            TransactionId::new([10; 32]),
            11,
            &fixture.state,
        )
        .await
    }

    #[tokio::test]
    async fn should_fail_construction_if_amount_is_zero() {
        let fixture = Fixture::new().await;

        let action = BridgeTransfer {
            amount: 0,
            ..new_bridge_transfer()
        };
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "amount must be greater than zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_withdrawal_event_id_empty() {
        let fixture = Fixture::new().await;

        let action = BridgeTransfer {
            rollup_withdrawal_event_id: String::new(),
            ..new_bridge_transfer()
        };
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup withdrawal event id must be non-empty");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_withdrawal_event_id_too_long() {
        let fixture = Fixture::new().await;

        let action = BridgeTransfer {
            rollup_withdrawal_event_id: ['a'; 257].into_iter().collect(),
            ..new_bridge_transfer()
        };
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "rollup withdrawal event id must not be more than 256 bytes",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_block_number_is_zero() {
        let fixture = Fixture::new().await;

        let action = BridgeTransfer {
            rollup_block_number: 0,
            ..new_bridge_transfer()
        };
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup block number must be greater than zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeTransfer {
            to: address_with_prefix([2; ADDRESS_LEN], prefix),
            ..new_bridge_transfer()
        };
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeTransfer {
            bridge_address: address_with_prefix([50; ADDRESS_LEN], prefix),
            ..new_bridge_transfer()
        };
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_account_not_initialized() {
        let fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "failed to get bridge account asset ID; account is not a bridge account",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_withdrawer_address_not_set() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        fixture
            .bridge_initializer(action.bridge_address)
            .with_no_withdrawer_address()
            .init();
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "bridge account must have a withdrawer address set");
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_authorized() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        fixture
            .bridge_initializer(action.bridge_address)
            .with_withdrawer_address([2; 20])
            .init();
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "signer is not the authorized withdrawer for the bridge account",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_withdrawal_event_id_already_used() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        fixture.bridge_initializer(action.bridge_address).init();
        let rollup_block_number = 999;
        let event_id = action.rollup_withdrawal_event_id.clone();
        fixture
            .state
            .put_withdrawal_event_block_for_bridge_account(
                &action.bridge_address,
                &event_id,
                rollup_block_number,
            )
            .unwrap();
        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("withdrawal event ID `{event_id}` used by block number {rollup_block_number}"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_asset_not_same_as_source() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        fixture.bridge_initializer(action.bridge_address).init();
        fixture
            .bridge_initializer(action.to)
            .with_asset(IbcPrefixed::new([10; 32]))
            .init();

        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "asset ID is not authorized for transfer to bridge account",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_rollup_id_not_found() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        fixture.bridge_initializer(action.bridge_address).init();
        fixture
            .bridge_initializer(action.to)
            .with_no_rollup_id()
            .init();

        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "bridge lock must be sent to a bridge account");
    }

    #[tokio::test]
    async fn should_fail_construction_if_asset_mapping_fails() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        let asset = IbcPrefixed::new([10; 32]);
        fixture
            .bridge_initializer(action.bridge_address)
            .with_asset(asset)
            .init();
        fixture
            .bridge_initializer(action.to)
            .with_asset(asset)
            .init();

        let err = new_checked_bridge_transfer(&fixture, action)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "mapping from IBC prefixed bridge asset to trace prefixed not found",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_authorized() {
        let mut fixture = Fixture::new().await;

        // Construct a checked bridge transfer while the tx signer is the authorized withdrawer.
        let action = new_bridge_transfer();
        fixture.bridge_initializer(action.bridge_address).init();
        fixture.bridge_initializer(action.to).init();
        let checked_action = new_checked_bridge_transfer(&fixture, action.clone())
            .await
            .unwrap();

        // Change the withdrawer address.
        let bridge_sudo_change = BridgeSudoChange {
            bridge_address: action.bridge_address,
            new_sudo_address: None,
            new_withdrawer_address: Some(astria_address(&[2; 20])),
            fee_asset: nria().into(),
        };
        let checked_bridge_sudo_change = CheckedAction::new_bridge_sudo_change(
            bridge_sudo_change,
            fixture.tx_signer,
            &fixture.state,
        )
        .await
        .unwrap();
        checked_bridge_sudo_change
            .execute(&mut fixture.state)
            .await
            .unwrap();

        // Try to execute the checked bridge transfer now - should fail due to tx signer no longer
        // being the authorized withdrawer.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "signer is not the authorized withdrawer for the bridge account",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_withdrawal_event_id_already_used() {
        let mut fixture = Fixture::new().await;

        // Construct a checked bridge transfer while the withdrawal event ID is unused.
        let action = new_bridge_transfer();
        fixture.bridge_initializer(action.bridge_address).init();
        fixture.bridge_initializer(action.to).init();
        let checked_action = new_checked_bridge_transfer(&fixture, action.clone())
            .await
            .unwrap();

        // Execute a bridge unlock with the same withdrawal event ID.
        let bridge_unlock = BridgeUnlock {
            to: astria_address(&[3; ADDRESS_LEN]),
            amount: 1,
            fee_asset: nria().into(),
            bridge_address: action.bridge_address,
            memo: "a".to_string(),
            rollup_block_number: 8,
            rollup_withdrawal_event_id: action.rollup_withdrawal_event_id.clone(),
        };
        let checked_bridge_unlock = CheckedAction::new_bridge_unlock(
            bridge_unlock.clone(),
            fixture.tx_signer,
            &fixture.state,
        )
        .await
        .unwrap();

        // Provide the bridge account with sufficient balance to execute the bridge unlock.
        fixture
            .state
            .increase_balance(&bridge_unlock.bridge_address, &nria(), bridge_unlock.amount)
            .await
            .unwrap();
        checked_bridge_unlock
            .execute(&mut fixture.state)
            .await
            .unwrap();

        // Try to execute the checked bridge transfer now with the same withdrawal event ID - should
        // fail due to the ID being used already.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!(
                "withdrawal event ID `{}` used by block number {}",
                action.rollup_withdrawal_event_id, bridge_unlock.rollup_block_number
            ),
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_bridge_account_has_insufficient_balance() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_transfer();
        fixture.bridge_initializer(action.bridge_address).init();
        fixture.bridge_initializer(action.to).init();
        let checked_action = new_checked_bridge_transfer(&fixture, action.clone())
            .await
            .unwrap();

        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "failed to decrease bridge account balance");
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;

        // Construct the checked bridge transfer while the account has insufficient balance to
        // ensure balance checks are only part of execution.
        let action = new_bridge_transfer();
        let rollup_id = RollupId::new([7; 32]);
        let asset = nria();
        fixture
            .bridge_initializer(action.bridge_address)
            .with_asset(asset.clone())
            .init();
        fixture
            .bridge_initializer(action.to)
            .with_rollup_id(rollup_id)
            .init();
        let checked_action = new_checked_bridge_transfer(&fixture, action.clone())
            .await
            .unwrap();

        // Provide the bridge account with sufficient balance.
        fixture
            .state
            .increase_balance(&action.bridge_address, &nria(), action.amount)
            .await
            .unwrap();

        // Check the balances are correct before execution.
        assert_eq!(
            fixture.get_nria_balance(&action.bridge_address).await,
            action.amount
        );
        assert_eq!(fixture.get_nria_balance(&action.to).await, 0);

        checked_action.execute(&mut fixture.state).await.unwrap();

        // Check the balances are correct after execution.
        assert_eq!(fixture.get_nria_balance(&action.bridge_address).await, 0);
        assert_eq!(fixture.get_nria_balance(&action.to).await, action.amount);

        // Check the rollup block number is recorded under the given event ID.
        let rollup_block_number = fixture
            .state
            .get_withdrawal_event_block_for_bridge_account(
                &action.bridge_address,
                &action.rollup_withdrawal_event_id,
            )
            .await
            .unwrap();
        assert_eq!(rollup_block_number, Some(action.rollup_block_number));

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
        assert_eq!(deposit.asset, Denom::from(asset));
        assert_eq!(
            deposit.destination_chain_address,
            action.destination_chain_address
        );
        assert_eq!(
            deposit.source_transaction_id,
            *checked_action.checked_bridge_lock.tx_id()
        );
        assert_eq!(
            deposit.source_action_index,
            checked_action.checked_bridge_lock.position_in_tx()
        );

        // Check the deposit event is cached.
        let deposit_events = fixture.state.flatten().1.take_events();
        assert_eq!(deposit_events.len(), 1);
        assert_eq!(deposit_events[0].kind, "tx.deposit");
    }
}
