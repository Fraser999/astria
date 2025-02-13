use astria_core::{
    primitive::v1::{
        asset::{
            Denom,
            IbcPrefixed,
        },
        Address,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::IbcRelayerChange,
};
use cnidarium::{
    Snapshot,
    StateDelta,
    TempStorage,
};
use futures::TryStreamExt as _;
use ibc_proto::{
    cosmos::ics23::v1::ProofSpec,
    google::protobuf::{
        Any,
        Duration,
        Timestamp,
    },
    ibc::{
        core::{
            client::v1::Height,
            commitment::v1::MerkleRoot,
        },
        lightclients::tendermint::v1::{
            ClientState,
            ConsensusState,
            Fraction,
        },
    },
};
use ibc_types::{
    core::client::{
        msgs::MsgCreateClient,
        ClientId,
    },
    lightclients::tendermint::{
        client_state::TENDERMINT_CLIENT_STATE_TYPE_URL,
        consensus_state::TENDERMINT_CONSENSUS_STATE_TYPE_URL,
    },
};
use penumbra_ibc::{
    component::ClientStateReadExt as _,
    IbcRelay,
};
use prost::Message as _;

use crate::{
    accounts::{
        AddressBytes,
        StateReadExt as _,
    },
    address::StateWriteExt as _,
    authority::StateWriteExt as _,
    benchmark_and_test_utils::{
        nria,
        ASTRIA_PREFIX,
    },
    fees::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

pub(super) fn test_asset() -> Denom {
    "test".parse().unwrap()
}

pub(super) fn address_with_prefix(address_bytes: [u8; ADDRESS_LEN], prefix: &str) -> Address {
    Address::builder()
        .array(address_bytes)
        .prefix(prefix)
        .try_build()
        .unwrap()
}

pub(super) fn new_client_state() -> ClientState {
    ClientState {
        chain_id: "abc-1".to_string(),
        trust_level: Some(Fraction {
            numerator: 1,
            denominator: 3,
        }),
        trusting_period: Some(Duration {
            seconds: 1,
            nanos: 0,
        }),
        unbonding_period: Some(Duration {
            seconds: 2,
            nanos: 0,
        }),
        max_clock_drift: Some(Duration {
            seconds: 1,
            nanos: 0,
        }),
        latest_height: Some(Height {
            revision_number: 1,
            revision_height: 1,
        }),
        proof_specs: vec![ProofSpec::default()],
        ..ClientState::default()
    }
}

pub(super) fn new_create_client() -> IbcRelay {
    let raw_client_state = new_client_state();
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

pub(super) struct Fixture {
    _storage: TempStorage,
    pub(super) state: StateDelta<Snapshot>,
    pub(super) tx_signer: [u8; ADDRESS_LEN],
}

impl Fixture {
    pub(super) async fn new() -> Self {
        let storage = TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let mut state = StateDelta::new(snapshot);
        state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
        let tx_signer = [1; ADDRESS_LEN];
        state.put_sudo_address(tx_signer).unwrap();
        state.put_allowed_fee_asset(&nria()).unwrap();
        Self {
            _storage: storage,
            state,
            tx_signer,
        }
    }

    pub(super) async fn allowed_fee_assets(&self) -> Vec<IbcPrefixed> {
        self.state.allowed_fee_assets().try_collect().await.unwrap()
    }

    pub(super) async fn get_nria_balance<TAddress: AddressBytes>(
        &self,
        address: &TAddress,
    ) -> u128 {
        self.state
            .get_account_balance(address, &nria())
            .await
            .unwrap()
    }
}
