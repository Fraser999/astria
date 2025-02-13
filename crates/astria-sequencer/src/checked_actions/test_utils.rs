use astria_core::primitive::v1::{
    asset::{
        Denom,
        IbcPrefixed,
    },
    Address,
    ADDRESS_LEN,
};
use cnidarium::{
    Snapshot,
    StateDelta,
    TempStorage,
};
use futures::TryStreamExt as _;

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
