use astria_core::{
    primitive::v1::{
        asset::IbcPrefixed,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::BridgeUnlock,
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
    bridge::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

pub(crate) type CheckedBridgeUnlock = CheckedBridgeUnlockImpl<true>;

/// This struct provides the implementation details of the checked bridge unlock action.
///
/// It is also used to perform checks on a bridge transfer action, which is essentially a cross
/// between a bridge unlock and a bridge lock.
///
/// A `BridgeUnlock` action does not allow unlocking to a bridge account, whereas `BridgeTransfer`
/// requires the `to` account to be a bridge one. Hence a bridge unlock is implemented via
/// `CheckedBridgeUnlockImpl<true>` and has methods to allow checking AND executing the action,
/// while the checks relevant to a bridge transfer are implemented via
/// `CheckedBridgeUnlockImpl<false>`, where this has no method supporting execution.
#[derive(Debug)]
pub(crate) struct CheckedBridgeUnlockImpl<const PURE_UNLOCK: bool> {
    action: BridgeUnlock,
    tx_signer: TransactionSignerAddressBytes,
    bridge_account_ibc_asset: IbcPrefixed,
}

impl<const PURE_UNLOCK: bool> CheckedBridgeUnlockImpl<PURE_UNLOCK> {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: BridgeUnlock,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // TODO(https://github.com/astriaorg/astria/issues/1430): move stateless checks to the
        // `BridgeUnlock` parsing.
        ensure!(action.amount > 0, "amount must be greater than zero");
        ensure!(
            action.memo.len() <= 64,
            "memo must not be more than 64 bytes"
        );
        ensure!(
            !action.rollup_withdrawal_event_id.is_empty(),
            "rollup withdrawal event id must be non-empty",
        );
        ensure!(
            action.rollup_withdrawal_event_id.len() <= 256,
            "rollup withdrawal event id must not be more than 256 bytes",
        );
        ensure!(
            action.rollup_block_number > 0,
            "rollup block number must be greater than zero",
        );

        state
            .ensure_base_prefix(&action.to)
            .await
            .wrap_err("destination address has an unsupported prefix")?;
        state
            .ensure_base_prefix(&action.bridge_address)
            .await
            .wrap_err("source address has an unsupported prefix")?;

        let bridge_account_ibc_asset = state
            .get_bridge_account_ibc_asset(&action.bridge_address)
            .await
            .wrap_err("failed to get bridge account asset ID; account is not a bridge account")?;

        let checked_action = Self {
            action,
            tx_signer: tx_signer.into(),
            bridge_account_ibc_asset,
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    pub(super) async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        if PURE_UNLOCK
            && state
                .is_a_bridge_account(&self.action.to)
                .await
                .wrap_err("failed to check if `to` address is a bridge account")?
        {
            bail!("bridge accounts cannot receive bridge unlocks");
        }

        let withdrawer = state
            .get_bridge_account_withdrawer_address(&self.action.bridge_address)
            .await
            .wrap_err("failed to get bridge account withdrawer address")?
            .ok_or_eyre("bridge account must have a withdrawer address set")?;
        ensure!(
            *self.tx_signer.as_bytes() == withdrawer,
            "signer is not the authorized withdrawer for the bridge account",
        );

        if let Some(existing_block_num) = state
            .get_withdrawal_event_block_for_bridge_account(
                &self.action.bridge_address,
                &self.action.rollup_withdrawal_event_id,
            )
            .await
            .wrap_err("failed to read withdrawal event block number from storage")?
        {
            bail!(
                "withdrawal event ID `{}` used by block number {existing_block_num}",
                self.action.rollup_withdrawal_event_id
            );
        }

        Ok(())
    }

    pub(super) fn record_withdrawal_event<S: StateWrite>(&self, mut state: S) -> Result<()> {
        state
            .put_withdrawal_event_block_for_bridge_account(
                &self.action.bridge_address,
                &self.action.rollup_withdrawal_event_id,
                self.action.rollup_block_number,
            )
            .wrap_err("failed to write withdrawal event block number to storage")
    }
}

impl CheckedBridgeUnlockImpl<true> {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;

        state
            .decrease_balance(
                &self.action.bridge_address,
                &self.bridge_account_ibc_asset,
                self.action.amount,
            )
            .await
            .wrap_err("failed to decrease bridge account balance")?;
        state
            .increase_balance(
                &self.action.to,
                &self.bridge_account_ibc_asset,
                self.action.amount,
            )
            .await
            .wrap_err("failed to increase destination account balance")?;

        self.record_withdrawal_event(state)
    }
}

impl CheckedBridgeUnlockImpl<false> {
    pub(super) fn action(&self) -> &BridgeUnlock {
        &self.action
    }

    pub(super) fn tx_signer(&self) -> &TransactionSignerAddressBytes {
        &self.tx_signer
    }

    pub(super) fn bridge_account_ibc_asset(&self) -> &IbcPrefixed {
        &self.bridge_account_ibc_asset
    }
}

// NOTE: unit tests here cover only `CheckedBridgeUnlockImpl<true>`.  Test coverage of
// `CheckedBridgeUnlockImpl<false>` is in `checked_actions::bridge_transfer`.
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
    use crate::benchmark_and_test_utils::{
        assert_eyre_error,
        astria_address,
        nria,
        ASTRIA_PREFIX,
    };

    fn new_bridge_unlock() -> BridgeUnlock {
        BridgeUnlock {
            to: astria_address(&[2; ADDRESS_LEN]),
            amount: 100,
            fee_asset: nria().into(),
            memo: "rollup memo".to_string(),
            bridge_address: astria_address(&[50; ADDRESS_LEN]),
            rollup_block_number: 10,
            rollup_withdrawal_event_id: "a-rollup-defined-hash".to_string(),
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_amount_is_zero() {
        let fixture = Fixture::new().await;

        let action = BridgeUnlock {
            amount: 0,
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "amount must be greater than zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_memo_too_long() {
        let fixture = Fixture::new().await;

        let action = BridgeUnlock {
            memo: ['a'; 65].into_iter().collect(),
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "memo must not be more than 64 bytes");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_withdrawal_event_id_empty() {
        let fixture = Fixture::new().await;

        let action = BridgeUnlock {
            rollup_withdrawal_event_id: String::new(),
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup withdrawal event id must be non-empty");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_withdrawal_event_id_too_long() {
        let fixture = Fixture::new().await;

        let action = BridgeUnlock {
            rollup_withdrawal_event_id: ['a'; 257].into_iter().collect(),
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
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

        let action = BridgeUnlock {
            rollup_block_number: 0,
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup block number must be greater than zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_destination_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = BridgeUnlock {
            to: address_with_prefix([2; ADDRESS_LEN], prefix),
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
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
        let action = BridgeUnlock {
            bridge_address: address_with_prefix([50; ADDRESS_LEN], prefix),
            ..new_bridge_unlock()
        };
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
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

        let action = new_bridge_unlock();
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "failed to get bridge account asset ID; account is not a bridge account",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_to_address_is_bridge_account() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_unlock();
        fixture.bridge_initializer(action.bridge_address).init();
        fixture.bridge_initializer(action.to).init();
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "bridge accounts cannot receive bridge unlocks");
    }

    #[tokio::test]
    async fn should_fail_construction_if_withdrawer_address_not_set() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_unlock();
        fixture
            .bridge_initializer(action.bridge_address)
            .with_no_withdrawer_address()
            .init();
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "bridge account must have a withdrawer address set");
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_is_not_authorized() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_unlock();
        fixture
            .bridge_initializer(action.bridge_address)
            .with_withdrawer_address([2; 20])
            .init();
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
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

        let action = new_bridge_unlock();
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
        let err = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("withdrawal event ID `{event_id}` used by block number {rollup_block_number}"),
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_to_address_is_bridge_account() {
        let mut fixture = Fixture::new().await;

        // Construct a checked bridge unlock while the `to` account is not a bridge account.
        let action = new_bridge_unlock();
        fixture.bridge_initializer(action.bridge_address).init();
        let to_address = action.to.bytes();
        let checked_action = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        // Initialize the `to` account as a bridge account.
        let init_bridge_account = InitBridgeAccount {
            rollup_id: RollupId::new([2; 32]),
            asset: "test".parse().unwrap(),
            fee_asset: "test".parse().unwrap(),
            sudo_address: None,
            withdrawer_address: None,
        };
        let checked_init_bridge_account =
            CheckedAction::new_init_bridge_account(init_bridge_account, to_address, &fixture.state)
                .await
                .unwrap();
        checked_init_bridge_account
            .execute(&mut fixture.state)
            .await
            .unwrap();

        // Try to execute the checked bridge unlock now - should fail due to `to` account now
        // existing.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(&err, "bridge accounts cannot receive bridge unlocks");
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_is_not_authorized() {
        let mut fixture = Fixture::new().await;

        // Construct a checked bridge unlock while the tx signer is the authorized withdrawer.
        let action = new_bridge_unlock();
        fixture.bridge_initializer(action.bridge_address).init();
        let checked_action =
            CheckedBridgeUnlock::new(action.clone(), fixture.tx_signer, &fixture.state)
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

        // Try to execute the checked bridge unlock now - should fail due to tx signer no longer
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

        // Construct two checked bridge unlocks while the withdrawal event ID has not been used.
        let action_1 = new_bridge_unlock();
        let event_id = action_1.rollup_withdrawal_event_id.clone();
        fixture.bridge_initializer(action_1.bridge_address).init();
        let checked_action_1 =
            CheckedBridgeUnlock::new(action_1.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let action_2 = BridgeUnlock {
            rollup_block_number: action_1.rollup_block_number.checked_add(1).unwrap(),
            ..action_1.clone()
        };
        let checked_action_2 =
            CheckedBridgeUnlock::new(action_2, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first bridge unlock to write the withdrawal event ID to state. Need to
        // provide the bridge account with sufficient balance.
        fixture
            .state
            .increase_balance(&action_1.bridge_address, &nria(), action_1.amount)
            .await
            .unwrap();
        checked_action_1.execute(&mut fixture.state).await.unwrap();

        // Try to execute the second checked bridge unlock now with the same withdrawal event ID -
        // should fail due to the ID being used already.
        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!(
                "withdrawal event ID `{event_id}` used by block number {}",
                action_1.rollup_block_number
            ),
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_bridge_account_has_insufficient_balance() {
        let mut fixture = Fixture::new().await;

        let action = new_bridge_unlock();
        fixture.bridge_initializer(action.bridge_address).init();
        let checked_action = CheckedBridgeUnlock::new(action, fixture.tx_signer, &fixture.state)
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

        // Construct the checked bridge unlock while the account has insufficient balance to ensure
        // balance checks are only part of execution.
        let action = new_bridge_unlock();
        fixture.bridge_initializer(action.bridge_address).init();
        let checked_action =
            CheckedBridgeUnlock::new(action.clone(), fixture.tx_signer, &fixture.state)
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
    }
}
