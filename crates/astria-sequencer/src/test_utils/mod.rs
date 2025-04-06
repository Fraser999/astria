use std::{
    fmt::Debug,
    sync::LazyLock,
    time::Duration,
};

use astria_core::{
    crypto::{
        SigningKey,
        ADDRESS_LENGTH,
    },
    primitive::v1::{
        Address,
        Bech32,
        RollupId,
    },
    protocol::transaction::v1::action::{
        BridgeLock,
        BridgeSudoChange,
        BridgeTransfer,
        BridgeUnlock,
        Ics20Withdrawal,
        InitBridgeAccount,
        RecoverIbcClient,
        RollupDataSubmission,
        Transfer,
    },
};
use bytes::Bytes;
use ibc_proto::{
    ibc::lightclients::tendermint::v1::ClientState as RawTmClientState,
    ics23::ProofSpec,
};
use ibc_types::{
    core::client::msgs::MsgCreateClient,
    lightclients::tendermint::{
        client_state::{
            AllowUpdate,
            ClientState,
            TENDERMINT_CLIENT_STATE_TYPE_URL,
        },
        consensus_state::TENDERMINT_CONSENSUS_STATE_TYPE_URL,
        TrustThreshold,
    },
};
use penumbra_ibc::IbcRelay;
use prost::Message as _;

pub(crate) use self::{
    bridge_initializer::BridgeInitializer,
    chain_initializer::ChainInitializer,
    fixture::Fixture,
    ics20_withdrawal_builder::Ics20WithdrawalBuilder,
};
use crate::benchmark_and_test_utils::{
    astria_address,
    nria,
    ASTRIA_COMPAT_PREFIX,
};

mod bridge_initializer;
mod chain_initializer;
mod fixture;
mod ics20_withdrawal_builder;

pub(crate) static ALICE: LazyLock<SigningKey> = LazyLock::new(|| SigningKey::from([1; 32]));
pub(crate) static ALICE_ADDRESS_BYTES: LazyLock<[u8; ADDRESS_LENGTH]> =
    LazyLock::new(|| ALICE.address_bytes());
pub(crate) static ALICE_ADDRESS: LazyLock<Address> =
    LazyLock::new(|| astria_address(&*ALICE_ADDRESS_BYTES));

pub(crate) static BOB: LazyLock<SigningKey> = LazyLock::new(|| SigningKey::from([2; 32]));
pub(crate) static BOB_ADDRESS_BYTES: LazyLock<[u8; ADDRESS_LENGTH]> =
    LazyLock::new(|| BOB.address_bytes());
pub(crate) static BOB_ADDRESS: LazyLock<Address> =
    LazyLock::new(|| astria_address(&*BOB_ADDRESS_BYTES));

pub(crate) static CAROL: LazyLock<SigningKey> = LazyLock::new(|| SigningKey::from([3; 32]));
pub(crate) static CAROL_ADDRESS_BYTES: LazyLock<[u8; ADDRESS_LENGTH]> =
    LazyLock::new(|| CAROL.address_bytes());
pub(crate) static CAROL_ADDRESS: LazyLock<Address> =
    LazyLock::new(|| astria_address(&*CAROL_ADDRESS_BYTES));

pub(crate) static SUDO: LazyLock<SigningKey> = LazyLock::new(|| SigningKey::from([100; 32]));
pub(crate) static SUDO_ADDRESS_BYTES: LazyLock<[u8; ADDRESS_LENGTH]> =
    LazyLock::new(|| SUDO.address_bytes());
pub(crate) static SUDO_ADDRESS: LazyLock<Address> =
    LazyLock::new(|| astria_address(&*SUDO_ADDRESS_BYTES));

pub(crate) static IBC_SUDO: LazyLock<SigningKey> = LazyLock::new(|| SigningKey::from([101; 32]));
pub(crate) static IBC_SUDO_ADDRESS_BYTES: LazyLock<[u8; ADDRESS_LENGTH]> =
    LazyLock::new(|| IBC_SUDO.address_bytes());
pub(crate) static IBC_SUDO_ADDRESS: LazyLock<Address> =
    LazyLock::new(|| astria_address(&*IBC_SUDO_ADDRESS_BYTES));

#[expect(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    reason = "allow is only necessary when benchmark isn't enabled"
)]
#[cfg_attr(feature = "benchmark", allow(dead_code))]
pub(crate) fn astria_compat_address(bytes: &[u8]) -> Address<Bech32> {
    Address::builder()
        .prefix(ASTRIA_COMPAT_PREFIX)
        .slice(bytes)
        .try_build()
        .unwrap()
}

/// Calculates the fee for a sequence `Action` based on the length of the `data`.
#[cfg(test)]
pub(crate) async fn calculate_rollup_data_submission_fee_from_state<
    S: crate::fees::StateReadExt,
>(
    data: &[u8],
    state: &S,
) -> u128 {
    let fees = state
        .get_fees::<RollupDataSubmission>()
        .await
        .expect("should not error fetching rollup data submission fees")
        .expect("rollup data submission fees should be stored");
    fees.base()
        .checked_add(
            fees.multiplier()
                .checked_mul(
                    data.len()
                        .try_into()
                        .expect("a usize should always convert to a u128"),
                )
                .expect("fee multiplication should not overflow"),
        )
        .expect("fee addition should not overflow")
}

pub(crate) fn borsh_then_hex<T: borsh::BorshSerialize>(item: &T) -> String {
    hex::encode(borsh::to_vec(item).unwrap())
}

pub(crate) fn dummy_bridge_lock() -> BridgeLock {
    BridgeLock {
        to: astria_address(&[50; ADDRESS_LENGTH]),
        amount: 100,
        asset: nria().into(),
        fee_asset: nria().into(),
        destination_chain_address: "test-chain".to_string(),
    }
}

pub(crate) fn dummy_bridge_sudo_change() -> BridgeSudoChange {
    BridgeSudoChange {
        bridge_address: astria_address(&[99; ADDRESS_LENGTH]),
        new_sudo_address: Some(astria_address(&[98; ADDRESS_LENGTH])),
        new_withdrawer_address: Some(astria_address(&[97; ADDRESS_LENGTH])),
        fee_asset: "test".parse().unwrap(),
    }
}

pub(crate) fn dummy_bridge_transfer() -> BridgeTransfer {
    BridgeTransfer {
        to: astria_address(&[99; ADDRESS_LENGTH]),
        amount: 100,
        fee_asset: nria().into(),
        destination_chain_address: "test-chain".to_string(),
        bridge_address: astria_address(&[50; ADDRESS_LENGTH]),
        rollup_block_number: 10,
        rollup_withdrawal_event_id: "a-rollup-defined-hash".to_string(),
    }
}

pub(crate) fn dummy_bridge_unlock() -> BridgeUnlock {
    BridgeUnlock {
        to: astria_address(&[3; ADDRESS_LENGTH]),
        amount: 100,
        fee_asset: nria().into(),
        memo: "rollup memo".to_string(),
        bridge_address: astria_address(&[50; ADDRESS_LENGTH]),
        rollup_block_number: 10,
        rollup_withdrawal_event_id: "a-rollup-defined-hash".to_string(),
    }
}

pub(crate) fn dummy_ibc_relay() -> IbcRelay {
    use ibc_proto::{
        google::protobuf::{
            Any,
            Timestamp,
        },
        ibc::{
            core::commitment::v1::MerkleRoot,
            lightclients::tendermint::v1::ConsensusState,
        },
    };

    let raw_client_state = RawTmClientState::from(dummy_ibc_client_state(1));
    let raw_consensus_state = ConsensusState {
        timestamp: Some(Timestamp {
            seconds: 1,
            nanos: 0,
        }),
        root: Some(MerkleRoot::default()),
        next_validators_hash: vec![],
    };
    IbcRelay::CreateClient(MsgCreateClient {
        client_state: Any {
            type_url: TENDERMINT_CLIENT_STATE_TYPE_URL.to_string(),
            value: raw_client_state.encode_to_vec(),
        },
        consensus_state: Any {
            type_url: TENDERMINT_CONSENSUS_STATE_TYPE_URL.to_string(),
            value: raw_consensus_state.encode_to_vec(),
        },
        signer: String::new(),
    })
}

pub(crate) fn dummy_ibc_client_state(rev_height: u64) -> ClientState {
    let version = 2;
    let chain_id = ibc_types::core::connection::ChainId::new("test".to_string(), version);
    let proof_spec = ProofSpec {
        leaf_spec: None,
        inner_spec: None,
        max_depth: 0,
        min_depth: 0,
        prehash_key_before_comparison: false,
    };
    let height = ibc_types::core::client::Height::new(version, rev_height).unwrap();
    let allow_update = AllowUpdate {
        after_expiry: true,
        after_misbehaviour: true,
    };
    ClientState::new(
        chain_id,
        TrustThreshold::TWO_THIRDS,
        Duration::from_secs(1),
        Duration::from_secs(64_000),
        Duration::from_secs(1),
        height,
        vec![proof_spec],
        vec![],
        allow_update,
        None,
    )
    .unwrap()
}

pub(crate) fn dummy_ics20_withdrawal() -> Ics20Withdrawal {
    Ics20WithdrawalBuilder::new().build()
}

pub(crate) fn dummy_init_bridge_account() -> InitBridgeAccount {
    InitBridgeAccount {
        rollup_id: RollupId::new([1; 32]),
        asset: "test".parse().unwrap(),
        fee_asset: "test".parse().unwrap(),
        sudo_address: Some(astria_address(&[2; ADDRESS_LENGTH])),
        withdrawer_address: Some(astria_address(&[3; ADDRESS_LENGTH])),
    }
}

pub(crate) fn dummy_recover_ibc_client() -> RecoverIbcClient {
    use ibc_types::core::client::{
        ClientId,
        ClientType,
    };

    RecoverIbcClient {
        client_id: ClientId::new(ClientType::new("test-id".to_string()), 0).unwrap(),
        replacement_client_id: ClientId::new(ClientType::new("test-id".to_string()), 1).unwrap(),
    }
}

pub(crate) fn dummy_rollup_data_submission() -> RollupDataSubmission {
    RollupDataSubmission {
        rollup_id: RollupId::new([1; 32]),
        data: Bytes::from(vec![1, 2, 3]),
        fee_asset: nria().into(),
    }
}

pub(crate) fn dummy_transfer() -> Transfer {
    Transfer {
        to: astria_address(&[50; ADDRESS_LENGTH]),
        fee_asset: nria().into(),
        asset: nria().into(),
        amount: 100,
    }
}

#[track_caller]
pub(crate) fn assert_error_contains<T: Debug>(error: &T, expected: &'_ str) {
    let msg = format!("{error:?}");
    assert!(
        msg.contains(expected),
        "error contained different message\n\texpected: {expected}\n\tfull_error: {msg}",
    );
}
