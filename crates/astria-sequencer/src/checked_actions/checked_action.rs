use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::{
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
        RollupDataSubmission,
        SudoAddressChange,
        Transfer,
        ValidatorUpdate,
    },
};
use astria_eyre::Result;
use cnidarium::{
    StateRead,
    StateWrite,
};
use penumbra_ibc::IbcRelay;

use super::{
    // CheckedBridgeLock,
    CheckedBridgeSudoChange,
    // CheckedBridgeTransfer,
    // CheckedBridgeUnlock,
    CheckedFeeAssetChange,
    CheckedFeeChange,
    CheckedIbcRelay,
    CheckedIbcRelayerChange,
    CheckedIbcSudoChange,
    CheckedIcs20Withdrawal,
    CheckedInitBridgeAccount,
    // CheckedRollupDataSubmission,
    CheckedSudoAddressChange,
    CheckedTransfer,
    // CheckedValidatorUpdate,
};

#[derive(Debug)]
pub(crate) enum CheckedAction {
    // RollupDataSubmission(CheckedRollupDataSubmission),
    Transfer(CheckedTransfer),
    // ValidatorUpdate(CheckedValidatorUpdate),
    SudoAddressChange(CheckedSudoAddressChange),
    IbcRelay(CheckedIbcRelay),
    IbcSudoChange(CheckedIbcSudoChange),
    Ics20Withdrawal(CheckedIcs20Withdrawal),
    IbcRelayerChange(CheckedIbcRelayerChange),
    FeeAssetChange(CheckedFeeAssetChange),
    InitBridgeAccount(CheckedInitBridgeAccount),
    // BridgeLock(CheckedBridgeLock),
    // BridgeUnlock(CheckedBridgeUnlock),
    BridgeSudoChange(CheckedBridgeSudoChange),
    // BridgeTransfer(CheckedBridgeTransfer),
    FeeChange(CheckedFeeChange),
}

impl CheckedAction {
    // pub(super) async fn new_rollup_data_submission<S: StateRead>(
    //     action: RollupDataSubmission,
    //     tx_signer: [u8; ADDRESS_LEN],
    //     state: S,
    // ) -> Result<Self> {
    //     let checked_action = CheckedRollupDataSubmission::new(action, tx_signer, state).await?;
    //     Ok(Self::RollupDataSubmission(checked_action))
    // }

    pub(super) async fn new_transfer<S: StateRead>(
        action: Transfer,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedTransfer::new(action, tx_signer, state).await?;
        Ok(Self::Transfer(checked_action))
    }

    // pub(super) async fn new_validator_update<S: StateRead>(
    //     action: ValidatorUpdate,
    //     tx_signer: [u8; ADDRESS_LEN],
    //     state: S,
    // ) -> Result<Self> {
    //     let checked_action = CheckedValidatorUpdate::new(action, tx_signer, state).await?;
    //     Ok(Self::ValidatorUpdate(checked_action))
    // }

    pub(super) async fn new_sudo_address_change<S: StateRead>(
        action: SudoAddressChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedSudoAddressChange::new(action, tx_signer, state).await?;
        Ok(Self::SudoAddressChange(checked_action))
    }

    pub(super) async fn new_ibc_relay<S: StateRead>(
        action: IbcRelay,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedIbcRelay::new(action, tx_signer, state).await?;
        Ok(Self::IbcRelay(checked_action))
    }

    pub(super) async fn new_ibc_sudo_change<S: StateRead>(
        action: IbcSudoChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedIbcSudoChange::new(action, tx_signer, state).await?;
        Ok(Self::IbcSudoChange(checked_action))
    }

    pub(super) async fn new_ics20_withdrawal<S: StateRead>(
        action: Ics20Withdrawal,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedIcs20Withdrawal::new(action, tx_signer, state).await?;
        Ok(Self::Ics20Withdrawal(checked_action))
    }

    pub(super) async fn new_ibc_relayer_change<S: StateRead>(
        action: IbcRelayerChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedIbcRelayerChange::new(action, tx_signer, state).await?;
        Ok(Self::IbcRelayerChange(checked_action))
    }

    pub(super) async fn new_fee_asset_change<S: StateRead>(
        action: FeeAssetChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedFeeAssetChange::new(action, tx_signer, state).await?;
        Ok(Self::FeeAssetChange(checked_action))
    }

    pub(super) async fn new_init_bridge_account<S: StateRead>(
        action: InitBridgeAccount,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedInitBridgeAccount::new(action, tx_signer, state).await?;
        Ok(Self::InitBridgeAccount(checked_action))
    }

    // pub(super) async fn new_bridge_lock<S: StateRead>(
    //     action: BridgeLock,
    //     tx_signer: [u8; ADDRESS_LEN],
    //     state: S,
    // ) -> Result<Self> {
    //     let checked_action = CheckedBridgeLock::new(action, tx_signer, state).await?;
    //     Ok(Self::BridgeLock(checked_action))
    // }

    // pub(super) async fn new_bridge_unlock<S: StateRead>(
    //     action: BridgeUnlock,
    //     tx_signer: [u8; ADDRESS_LEN],
    //     state: S,
    // ) -> Result<Self> {
    //     let checked_action = CheckedBridgeUnlock::new(action, tx_signer, state).await?;
    //     Ok(Self::BridgeUnlock(checked_action))
    // }

    pub(super) async fn new_bridge_sudo_change<S: StateRead>(
        action: BridgeSudoChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedBridgeSudoChange::new(action, tx_signer, state).await?;
        Ok(Self::BridgeSudoChange(checked_action))
    }

    // pub(super) async fn new_bridge_transfer<S: StateRead>(
    //     action: BridgeTransfer,
    //     tx_signer: [u8; ADDRESS_LEN],
    //     state: S,
    // ) -> Result<Self> {
    //     let checked_action = CheckedBridgeTransfer::new(action, tx_signer, state).await?;
    //     Ok(Self::BridgeTransfer(checked_action))
    // }

    pub(super) async fn new_fee_change<S: StateRead>(
        action: FeeChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = CheckedFeeChange::new(action, tx_signer, state).await?;
        Ok(Self::FeeChange(checked_action))
    }

    pub(super) async fn execute<S: StateWrite>(&self, state: S) -> Result<()> {
        match self {
            // Self::RollupDataSubmission(checked_action) => checked_action.execute(state).await,
            Self::Transfer(checked_action) => checked_action.execute(state).await,
            // Self::ValidatorUpdate(checked_action) => checked_action.execute(state).await,
            Self::SudoAddressChange(checked_action) => checked_action.execute(state).await,
            Self::IbcRelay(checked_action) => checked_action.execute(state).await,
            Self::IbcSudoChange(checked_action) => checked_action.execute(state).await,
            Self::Ics20Withdrawal(checked_action) => checked_action.execute(state).await,
            Self::IbcRelayerChange(checked_action) => checked_action.execute(state).await,
            Self::FeeAssetChange(checked_action) => checked_action.execute(state).await,
            Self::InitBridgeAccount(checked_action) => checked_action.execute(state).await,
            // Self::BridgeLock(checked_action) => checked_action.execute(state).await,
            // Self::BridgeUnlock(checked_action) => checked_action.execute(state).await,
            Self::BridgeSudoChange(checked_action) => checked_action.execute(state).await,
            // Self::BridgeTransfer(checked_action) => checked_action.execute(state).await,
            Self::FeeChange(checked_action) => checked_action.execute(state).await,
        }
    }
}
