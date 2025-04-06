use astria_core::primitive::v1::{
    asset::Denom,
    Address,
    RollupId,
    ADDRESS_LEN,
};
use cnidarium::{
    Snapshot,
    StateDelta,
};

use super::{
    Fixture,
    SUDO_ADDRESS,
};
use crate::{
    accounts::AddressBytes,
    benchmark_and_test_utils::nria,
    bridge::StateWriteExt as _,
};

pub(crate) struct BridgeInitializer<'a> {
    state: &'a mut StateDelta<Snapshot>,
    bridge_address: Address,
    rollup_id: Option<RollupId>,
    asset: Denom,
    sudo_address: [u8; ADDRESS_LEN],
    withdrawer_address: Option<[u8; ADDRESS_LEN]>,
}

impl<'a> BridgeInitializer<'a> {
    pub(super) fn new(fixture: &'a mut Fixture, bridge_address: Address) -> Self {
        Self {
            state: fixture.state_mut(),
            bridge_address,
            rollup_id: Some(RollupId::new([1; 32])),
            asset: nria().into(),
            sudo_address: *SUDO_ADDRESS.address_bytes(),
            withdrawer_address: Some(*SUDO_ADDRESS.address_bytes()),
        }
    }

    pub(crate) fn with_asset<T: Into<Denom>>(mut self, asset: T) -> Self {
        self.asset = asset.into();
        self
    }

    pub(crate) fn with_rollup_id(mut self, rollup_id: RollupId) -> Self {
        self.rollup_id = Some(rollup_id);
        self
    }

    pub(crate) fn with_no_rollup_id(mut self) -> Self {
        self.rollup_id = None;
        self
    }

    pub(crate) fn with_withdrawer_address(mut self, withdrawer_address: [u8; ADDRESS_LEN]) -> Self {
        self.withdrawer_address = Some(withdrawer_address);
        self
    }

    pub(crate) fn with_no_withdrawer_address(mut self) -> Self {
        self.withdrawer_address = None;
        self
    }

    pub(crate) fn init(self) {
        if let Some(rollup_id) = self.rollup_id {
            self.state
                .put_bridge_account_rollup_id(&self.bridge_address, rollup_id)
                .unwrap();
        }
        self.state
            .put_bridge_account_ibc_asset(&self.bridge_address, &self.asset)
            .unwrap();
        self.state
            .put_bridge_account_sudo_address(&self.bridge_address, self.sudo_address)
            .unwrap();
        if let Some(withdrawer_address) = self.withdrawer_address {
            self.state
                .put_bridge_account_withdrawer_address(&self.bridge_address, withdrawer_address)
                .unwrap();
        }
    }
}
