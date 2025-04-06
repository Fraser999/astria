use astria_core::{
    crypto::VerificationKey,
    primitive::v1::{
        asset::{
            Denom,
            IbcPrefixed,
        },
        Address,
        RollupId,
        ADDRESS_LEN,
    },
    protocol::{
        fees::v1::FeeComponents,
        transaction::v1::{
            action::{
                BridgeLock,
                BridgeSudoChange,
                FeeAssetChange,
                FeeChange,
                IbcRelayerChange,
                IbcSudoChange,
                RollupDataSubmission,
                SudoAddressChange,
                Transfer,
                ValidatorUpdate,
            },
            Action,
        },
    },
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
    core::client::msgs::MsgCreateClient,
    lightclients::tendermint::{
        client_state::TENDERMINT_CLIENT_STATE_TYPE_URL,
        consensus_state::TENDERMINT_CONSENSUS_STATE_TYPE_URL,
    },
};
use penumbra_ibc::IbcRelay;
use prost::Message as _;

use crate::{
    accounts::{
        AddressBytes,
        StateReadExt as _,
    },
    address::StateWriteExt as _,
    assets::StateWriteExt as _,
    authority::StateWriteExt as _,
    benchmark_and_test_utils::{
        astria_address,
        nria,
        ASTRIA_PREFIX,
    },
    bridge::StateWriteExt as _,
    fees::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    test_utils::{
        dummy_bridge_lock,
        dummy_bridge_sudo_change,
        dummy_bridge_transfer,
        dummy_bridge_unlock,
        dummy_ibc_relay,
        dummy_ics20_withdrawal,
        dummy_init_bridge_account,
        dummy_recover_ibc_client,
        dummy_rollup_data_submission,
        dummy_transfer,
    },
};

pub(crate) fn dummy_actions() -> [Action; 16] {
    let validator_update = ValidatorUpdate {
        power: 101,
        verification_key: VerificationKey::try_from([10; 32]).unwrap(),
    };
    let sudo_address_change = SudoAddressChange {
        new_address: astria_address(&[2; ADDRESS_LEN]),
    };
    let ibc_sudo_change = IbcSudoChange {
        new_address: astria_address(&[2; ADDRESS_LEN]),
    };
    let ibc_relayer_change = IbcRelayerChange::Addition(astria_address(&[50; ADDRESS_LEN]));
    let fee_asset_change = FeeAssetChange::Addition("test".parse().unwrap());
    let fee_change = FeeChange::Transfer(FeeComponents::new(1, 2));

    [
        Action::RollupDataSubmission(dummy_rollup_data_submission()),
        Action::Transfer(dummy_transfer()),
        Action::ValidatorUpdate(validator_update),
        Action::SudoAddressChange(sudo_address_change),
        Action::Ibc(dummy_ibc_relay()),
        Action::IbcSudoChange(ibc_sudo_change),
        Action::Ics20Withdrawal(dummy_ics20_withdrawal()),
        Action::IbcRelayerChange(ibc_relayer_change),
        Action::FeeAssetChange(fee_asset_change),
        Action::InitBridgeAccount(dummy_init_bridge_account()),
        Action::BridgeLock(dummy_bridge_lock()),
        Action::BridgeUnlock(dummy_bridge_unlock()),
        Action::BridgeSudoChange(dummy_bridge_sudo_change()),
        Action::BridgeTransfer(dummy_bridge_transfer()),
        Action::FeeChange(fee_change),
        Action::RecoverIbcClient(dummy_recover_ibc_client()),
    ]
}

pub(super) fn address_with_prefix(address_bytes: [u8; ADDRESS_LEN], prefix: &str) -> Address {
    Address::builder()
        .array(address_bytes)
        .prefix(prefix)
        .try_build()
        .unwrap()
}
