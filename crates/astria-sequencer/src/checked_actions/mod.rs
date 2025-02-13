// mod bridge_lock;
mod bridge_sudo_change;
// mod bridge_transfer;
// mod bridge_unlock;
mod checked_action;
mod fee_asset_change;
mod fee_change;
mod ibc_relay;
mod ibc_relayer_change;
mod ibc_sudo_change;
mod ics20_withdrawal;
mod init_bridge_account;
// mod rollup_data_submission;
mod sudo_address_change;
#[cfg(test)]
mod test_utils;
mod transfer;
// mod validator_update;

use std::fmt::{
    self,
    Debug,
    Formatter,
};

use astria_core::crypto::ADDRESS_LENGTH;
// pub(crate) use bridge_lock::CheckedBridgeLock;
pub(crate) use bridge_sudo_change::CheckedBridgeSudoChange;
// pub(crate) use validator_update::CheckedValidatorUpdate;
pub(crate) use checked_action::CheckedAction;
// pub(crate) use bridge_transfer::CheckedBridgeTransfer;
// pub(crate) use bridge_unlock::CheckedBridgeUnlock;
pub(crate) use fee_asset_change::CheckedFeeAssetChange;
pub(crate) use fee_change::CheckedFeeChange;
pub(crate) use ibc_relay::CheckedIbcRelay;
pub(crate) use ibc_relayer_change::CheckedIbcRelayerChange;
pub(crate) use ibc_sudo_change::CheckedIbcSudoChange;
pub(crate) use ics20_withdrawal::CheckedIcs20Withdrawal;
pub(crate) use init_bridge_account::CheckedInitBridgeAccount;
// pub(crate) use rollup_data_submission::CheckedRollupDataSubmission;
pub(crate) use sudo_address_change::CheckedSudoAddressChange;
pub(crate) use transfer::CheckedTransfer;

use crate::accounts::AddressBytes;

struct TransactionSignerAddressBytes([u8; ADDRESS_LENGTH]);

impl TransactionSignerAddressBytes {
    #[must_use]
    fn as_bytes(&self) -> &[u8; ADDRESS_LENGTH] {
        &self.0
    }
}

impl From<[u8; ADDRESS_LENGTH]> for TransactionSignerAddressBytes {
    fn from(address_bytes: [u8; ADDRESS_LENGTH]) -> Self {
        Self(address_bytes)
    }
}

impl Debug for TransactionSignerAddressBytes {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", telemetry::display::base64(&self.0))
    }
}

impl AddressBytes for TransactionSignerAddressBytes {
    fn address_bytes(&self) -> &[u8; ADDRESS_LENGTH] {
        &self.0
    }
}
