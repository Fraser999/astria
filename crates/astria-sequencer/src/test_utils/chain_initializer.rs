use astria_core::{
    crypto::VerificationKey,
    generated::astria::protocol::genesis::v1::{
        Account as RawAccount,
        AddressPrefixes as RawAddressPrefixes,
        GenesisAppState as RawGenesisAppState,
        IbcParameters as RawIbcParameters,
    },
    protocol::{
        fees::v1::FeeComponents,
        genesis::v1::{
            GenesisAppState,
            GenesisFees,
        },
        transaction::v1::action::ValidatorUpdate,
    },
    Protobuf as _,
};

use super::{
    Fixture,
    ALICE,
    ALICE_ADDRESS,
    BOB,
    BOB_ADDRESS,
    CAROL,
    CAROL_ADDRESS,
    IBC_SUDO_ADDRESS,
    SUDO_ADDRESS,
};
use crate::benchmark_and_test_utils::{
    nria,
    ASTRIA_COMPAT_PREFIX,
    ASTRIA_PREFIX,
};

pub(crate) struct ChainInitializer<'a> {
    fixture: &'a mut Fixture,
    raw_genesis_app_state: RawGenesisAppState,
    genesis_validators: Vec<ValidatorUpdate>,
}

impl<'a> ChainInitializer<'a> {
    pub(super) fn new(fixture: &'a mut Fixture) -> Self {
        let genesis_validators = vec![
            ValidatorUpdate {
                power: 10,
                verification_key: ALICE.verification_key(),
            },
            ValidatorUpdate {
                power: 10,
                verification_key: BOB.verification_key(),
            },
            ValidatorUpdate {
                power: 10,
                verification_key: CAROL.verification_key(),
            },
        ];
        Self {
            fixture,
            raw_genesis_app_state: dummy_genesis_state(),
            genesis_validators,
        }
    }

    pub(crate) fn with_no_fees(mut self) -> Self {
        self.raw_genesis_app_state.fees = Some(
            GenesisFees {
                rollup_data_submission: None,
                transfer: None,
                ics20_withdrawal: None,
                init_bridge_account: None,
                bridge_lock: None,
                bridge_unlock: None,
                bridge_transfer: None,
                bridge_sudo_change: None,
                ibc_relay: None,
                validator_update: None,
                fee_asset_change: None,
                fee_change: FeeComponents::new(0, 0),
                ibc_relayer_change: None,
                sudo_address_change: None,
                ibc_sudo_change: None,
                recover_ibc_client: None,
            }
            .to_raw(),
        );
        self
    }

    pub(crate) fn with_genesis_validators<I: IntoIterator<Item = (VerificationKey, u32)>>(
        mut self,
        validators: I,
    ) -> Self {
        self.genesis_validators = validators
            .into_iter()
            .map(|(verification_key, power)| ValidatorUpdate {
                power,
                verification_key,
            })
            .collect();
        self
    }

    pub(crate) async fn init(self) {
        let ChainInitializer {
            fixture,
            raw_genesis_app_state,
            genesis_validators,
        } = self;

        if fixture.genesis_app_state.is_some() {
            panic!("can only init chain once");
        }

        let genesis_app_state = GenesisAppState::try_from_raw(raw_genesis_app_state).unwrap();
        fixture
            .app
            .init_chain(
                fixture.storage.clone(),
                genesis_app_state.clone(),
                genesis_validators,
                genesis_app_state.chain_id().to_string(),
            )
            .await
            .unwrap();
        fixture.app.commit(fixture.storage.clone()).await;

        fixture.genesis_app_state = Some(genesis_app_state)
    }
}

fn dummy_genesis_state() -> RawGenesisAppState {
    let address_prefixes = RawAddressPrefixes {
        base: ASTRIA_PREFIX.into(),
        ibc_compat: ASTRIA_COMPAT_PREFIX.into(),
    };
    let accounts = vec![
        RawAccount {
            address: Some(ALICE_ADDRESS.to_raw()),
            balance: Some(10_u128.pow(19).into()),
        },
        RawAccount {
            address: Some(BOB_ADDRESS.to_raw()),
            balance: Some(10_u128.pow(19).into()),
        },
        RawAccount {
            address: Some(CAROL_ADDRESS.to_raw()),
            balance: Some(10_u128.pow(19).into()),
        },
    ];
    let ibc_parameters = RawIbcParameters {
        ibc_enabled: true,
        inbound_ics20_transfers_enabled: true,
        outbound_ics20_transfers_enabled: true,
    };

    RawGenesisAppState {
        chain_id: "test".to_string(),
        address_prefixes: Some(address_prefixes),
        accounts,
        authority_sudo_address: Some(SUDO_ADDRESS.to_raw()),
        ibc_sudo_address: Some(IBC_SUDO_ADDRESS.to_raw()),
        ibc_relayer_addresses: vec![IBC_SUDO_ADDRESS.to_raw()],
        native_asset_base_denomination: nria().to_string(),
        ibc_parameters: Some(ibc_parameters),
        allowed_fee_assets: vec![nria().to_string()],
        fees: Some(dummy_genesis_fees().to_raw()),
    }
}

fn dummy_genesis_fees() -> GenesisFees {
    GenesisFees {
        rollup_data_submission: Some(FeeComponents::new(1, 1001)),
        transfer: Some(FeeComponents::new(2, 1002)),
        ics20_withdrawal: Some(FeeComponents::new(3, 1003)),
        init_bridge_account: Some(FeeComponents::new(4, 1004)),
        bridge_lock: Some(FeeComponents::new(5, 1005)),
        bridge_unlock: Some(FeeComponents::new(6, 1006)),
        bridge_transfer: Some(FeeComponents::new(7, 1007)),
        bridge_sudo_change: Some(FeeComponents::new(8, 1008)),
        ibc_relay: Some(FeeComponents::new(9, 1009)),
        validator_update: Some(FeeComponents::new(10, 1010)),
        fee_asset_change: Some(FeeComponents::new(11, 1011)),
        fee_change: FeeComponents::new(12, 1012),
        ibc_relayer_change: Some(FeeComponents::new(13, 1013)),
        sudo_address_change: Some(FeeComponents::new(14, 1014)),
        ibc_sudo_change: Some(FeeComponents::new(15, 1015)),
        recover_ibc_client: Some(FeeComponents::new(16, 1016)),
    }
}
