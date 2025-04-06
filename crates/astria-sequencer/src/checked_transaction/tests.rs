use astria_core::{
    primitive::v1::{
        asset::TracePrefixed,
        RollupId,
    },
    protocol::{
        fees::v1::FeeComponents,
        genesis::v1::GenesisFees,
        transaction::v1::{
            action::{
                BridgeLock,
                BridgeSudoChange,
                BridgeTransfer,
                BridgeUnlock,
                FeeAssetChange,
                FeeChange,
                IbcRelayerChange,
                IbcSudoChange,
                Ics20Withdrawal,
                InitBridgeAccount,
                RecoverIbcClient,
                RollupDataSubmission,
                SudoAddressChange,
                Transfer,
                ValidatorUpdate,
            },
            TransactionBody,
        },
    },
};
use cnidarium::{
    Snapshot,
    StateDelta,
    TempStorage,
};
use penumbra_ibc::IbcRelay;
use telemetry::Metrics as _;

use super::*;
use crate::{
    address::StateWriteExt as _,
    app::{
        benchmark_and_test_utils::default_fees,
        test_utils::get_alice_signing_key,
        StateWriteExt as _,
    },
    assets::StateWriteExt as _,
    authority::StateWriteExt as _,
    benchmark_and_test_utils::{
        astria_address,
        nria,
        ASTRIA_PREFIX,
    },
    checked_actions::test_utils::dummy_actions,
    fees::StateWriteExt as _,
    ibc::StateWriteExt as _,
    test_utils::{
        dummy_bridge_lock,
        dummy_bridge_transfer,
        dummy_ics20_withdrawal,
        Fixture,
        SUDO,
    },
    Metrics,
};

fn actions_filtered_by_group(actions: &[Action], group: Group) -> Vec<Action> {
    actions
        .iter()
        .filter(|action| action.group() == group)
        .cloned()
        .collect()
}

// impl Fixture {
//     pub(super) async fn new(fees: GenesisFees) -> Self {
//         let storage = TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state
//             .put_chain_id_and_revision_number("test".parse().unwrap())
//             .unwrap();
//         let tx_signer = *get_alice_signing_key().verification_key().address_bytes();
//         state.put_sudo_address(tx_signer).unwrap();
//         state.put_ibc_sudo_address(tx_signer).unwrap();
//         state.put_ibc_relayer_address(&tx_signer).unwrap();
//         state.put_allowed_fee_asset(&nria()).unwrap();
//         state.put_ibc_asset(nria()).unwrap();
//         state.put_block_height(1).unwrap();
//         state.put_revision_number(1).unwrap();
//         let timestamp = tendermint::Time::from_unix_timestamp(1, 0).unwrap();
//         state.put_block_timestamp(timestamp).unwrap();
//         let rollup_id = RollupId::new([1; 32]);
//         let Action::BridgeLock(ref bridge_lock) = dummy_actions()[10] else {
//             panic!("should be bridge lock");
//         };
//         state
//             .put_bridge_account_ibc_asset(&bridge_lock.to, nria())
//             .unwrap();
//         state
//             .put_bridge_account_rollup_id(&bridge_lock.to, rollup_id)
//             .unwrap();
//         state
//             .put_bridge_account_sudo_address(&bridge_lock.to, tx_signer)
//             .unwrap();
//         state
//             .put_bridge_account_withdrawer_address(&bridge_lock.to, tx_signer)
//             .unwrap();
//         let Action::BridgeUnlock(ref bridge_unlock) = dummy_actions()[11] else {
//             panic!("should be bridge unlock");
//         };
//         state
//             .put_bridge_account_ibc_asset(&bridge_unlock.bridge_address, nria())
//             .unwrap();
//         state
//             .put_bridge_account_rollup_id(&bridge_unlock.bridge_address, rollup_id)
//             .unwrap();
//         state
//             .put_bridge_account_sudo_address(&bridge_unlock.bridge_address, tx_signer)
//             .unwrap();
//         state
//             .put_bridge_account_withdrawer_address(&bridge_unlock.bridge_address, tx_signer)
//             .unwrap();
//         let Action::BridgeTransfer(ref bridge_transfer) = dummy_actions()[13] else {
//             panic!("should be bridge transfer");
//         };
//         state
//             .put_bridge_account_ibc_asset(&bridge_transfer.to, nria())
//             .unwrap();
//         state
//             .put_bridge_account_rollup_id(&bridge_transfer.to, rollup_id)
//             .unwrap();
//         state
//             .put_bridge_account_sudo_address(&bridge_transfer.to, tx_signer)
//             .unwrap();
//         state
//             .put_bridge_account_withdrawer_address(&bridge_transfer.to, tx_signer)
//             .unwrap();
//         if let Some(fee) = fees.rollup_data_submission {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.transfer {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.validator_update {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.sudo_address_change {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.ibc_relay {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.ibc_sudo_change {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.ics20_withdrawal {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.ibc_relayer_change {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.fee_asset_change {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.init_bridge_account {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.bridge_lock {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.bridge_unlock {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.bridge_sudo_change {
//             state.put_fees(fee).unwrap();
//         }
//         if let Some(fee) = fees.bridge_transfer {
//             state.put_fees(fee).unwrap();
//         }
//         state.put_fees(fees.fee_change).unwrap();
//         if let Some(fee) = fees.recover_ibc_client {
//             state.put_fees(fee).unwrap();
//         }
//
//         let metrics = Box::leak(Box::new(Metrics::noop_metrics(&()).unwrap()));
//
//         Self {
//             _storage: storage,
//             state,
//             tx_signer,
//             metrics,
//         }
//     }
// }

#[tokio::test]
async fn should_calculate_total_cost() {
    todo!("maybe ticket for follow-up, concentrate on construction failures and porting app tests");
    let mut fixture = Fixture::default_initialized().await;
    fixture.bridge_initializer(dummy_bridge_lock().to).init();
    fixture
        .bridge_initializer(dummy_bridge_transfer().bridge_address)
        .init();
    fixture
        .bridge_initializer(dummy_bridge_transfer().to)
        .init();
    fixture
        .state_mut()
        .put_allowed_fee_asset(&dummy_ics20_withdrawal().fee_asset)
        .unwrap();
    let metrics = fixture.metrics();

    let actions = dummy_actions()
        .iter()
        .filter(|action| {
            action.group() == Group::BundleableGeneral && !matches!(action, Action::Ibc(_))
        })
        .cloned()
        .collect();

    let tx_bytes: Bytes = TransactionBody::builder()
        .actions(actions)
        .chain_id(fixture.chain_id())
        .nonce(1)
        .try_build()
        .unwrap()
        .sign(&*SUDO)
        .into_raw()
        .encode_to_vec()
        .into();
    let checked_tx = CheckedTransaction::new([1; 32], tx_bytes, fixture.state_mut(), metrics)
        .await
        .unwrap();
    let total_cost = checked_tx.total_costs(fixture.state_mut()).await.unwrap();
    println!("{total_cost:?}");
    let tx_bytes: Bytes = TransactionBody::builder()
        .actions(actions_filtered_by_group(
            &dummy_actions(),
            Group::BundleableSudo,
        ))
        .chain_id(fixture.chain_id())
        .nonce(2)
        .try_build()
        .unwrap()
        .sign(&*SUDO)
        .into_raw()
        .encode_to_vec()
        .into();
    let checked_tx = CheckedTransaction::new([2; 32], tx_bytes, fixture.state_mut(), metrics)
        .await
        .unwrap();
    let total_cost = checked_tx.total_costs(fixture.state_mut()).await.unwrap();
    println!("{total_cost:?}");
    todo!();
}

#[test]
fn toodoo() {
    todo!("move tests from `src/transaction/checks.rs`");
    todo!("move tests from `src/app/tests_execute_transaction.rs`");
}
