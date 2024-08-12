use anyhow::{
    bail,
    Context,
    Result,
};
use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1alpha1::action::TransferAction,
    Protobuf,
};
use cnidarium::{
    StateRead,
    StateWrite,
};

use super::AddressBytes;
use crate::{
    accounts::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    address::StateReadExt as _,
    app::ActionHandler,
    assets::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    bridge::StateReadExt as _,
    immutable_data::ImmutableData,
    // transaction::StateReadExt as _,
};

#[async_trait::async_trait]
impl ActionHandler for TransferAction {
    type CheckStatelessContext = ();

    async fn check_stateless(&self, _context: Self::CheckStatelessContext) -> Result<()> {
        Ok(())
    }

    async fn check_and_execute<S: StateWrite>(
        &self,
        state: S,
        immutable_data: &ImmutableData,
        from: [u8; 20],
    ) -> Result<()> {
        tokio::try_join!(
            async {
                if state
                    .get_bridge_account_rollup_id(from)
                    .await
                    .context("failed to get bridge account rollup id")?
                    .is_some()
                {
                    bail!("cannot transfer out of bridge account; BridgeUnlock must be used")
                }
                Ok(())
            },
            check_transfer(self, from, &state, immutable_data),
        )?;
        execute_transfer(self, from, state, immutable_data).await
    }
}

pub(crate) async fn execute_transfer<S: StateWrite>(
    action: &TransferAction,
    from: [u8; ADDRESS_LEN],
    mut state: S,
    immutable_data: &ImmutableData,
) -> anyhow::Result<()> {
    let fee = state.get_transfer_base_fee(immutable_data);
    state
        .get_and_increase_block_fees(&action.fee_asset, fee, TransferAction::full_name())
        .await
        .context("failed to add to block fees")?;

    // if fee payment asset is same asset as transfer asset, deduct fee
    // from same balance as asset transferred
    if action.asset.to_ibc_prefixed() == action.fee_asset.to_ibc_prefixed() {
        // check_stateful should have already checked this arithmetic
        let payment_amount = action
            .amount
            .checked_add(fee)
            .expect("transfer amount plus fee should not overflow");

        state
            .decrease_balance(from, &action.asset, payment_amount)
            .await
            .context("failed decreasing `from` account balance")?;
        state
            .increase_balance(action.to, &action.asset, action.amount)
            .await
            .context("failed increasing `to` account balance")?;
    } else {
        // otherwise, just transfer the transfer asset and deduct fee from fee asset balance
        // later
        state
            .decrease_balance(from, &action.asset, action.amount)
            .await
            .context("failed decreasing `from` account balance")?;
        state
            .increase_balance(action.to, &action.asset, action.amount)
            .await
            .context("failed increasing `to` account balance")?;

        // deduct fee from fee asset balance
        state
            .decrease_balance(from, &action.fee_asset, fee)
            .await
            .context("failed decreasing `from` account balance for fee payment")?;
    }
    Ok(())
}

pub(crate) async fn check_transfer<S, TAddress>(
    action: &TransferAction,
    _from: TAddress,
    state: &S,
    immutable_data: &ImmutableData,
) -> Result<()>
where
    S: StateRead,
    TAddress: AddressBytes,
{
    tokio::try_join!(
        state.ensure_base_prefix(&action.to, immutable_data),
        async {
            if !state
                .is_allowed_fee_asset(&action.fee_asset)
                .await
                .context("failed to check allowed fee assets in state")?
            {
                bail!("more bad stuffs");
            }
            Ok(())
        }
    )?;

    // let fee = state.get_transfer_base_fee(immutable_data);
    // let transfer_asset = action.asset.clone();
    //
    // let from_fee_balance = state
    //     .get_account_balance(&from, &action.fee_asset)
    //     .await
    //     .context("failed getting `from` account balance for fee payment")?;
    //
    // // if fee asset is same as transfer asset, ensure accounts has enough funds
    // // to cover both the fee and the amount transferred
    // if action.fee_asset.to_ibc_prefixed() == transfer_asset.to_ibc_prefixed() {
    //     let payment_amount = action
    //         .amount
    //         .checked_add(fee)
    //         .context("transfer amount plus fee overflowed")?;
    //
    //     ensure!(
    //         from_fee_balance >= payment_amount,
    //         "insufficient funds for transfer and fee payment"
    //     );
    // } else {
    //     // otherwise, check the fee asset account has enough to cover the fees,
    //     // and the transfer asset account has enough to cover the transfer
    //     ensure!(
    //         from_fee_balance >= fee,
    //         "insufficient funds for fee payment"
    //     );
    //
    //     let from_transfer_balance = state
    //         .get_account_balance(from, transfer_asset)
    //         .await
    //         .context("failed to get account balance in transfer check")?;
    //     ensure!(
    //         from_transfer_balance >= action.amount,
    //         "insufficient funds for transfer"
    //     );
    // }

    Ok(())
}
