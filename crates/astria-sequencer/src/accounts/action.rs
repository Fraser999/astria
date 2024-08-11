use anyhow::{
    ensure,
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
    cache::Cache,
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
        from: [u8; 20],
        state: S,
        cache: &Cache,
    ) -> Result<()> {
        let mut s = std::time::Instant::now();
        let s1 = s;
        // let from = state
        //     .get_current_source()
        //     .expect("transaction source must be present in state when executing an action")
        //     .address_bytes();

        ensure!(
            state
                .get_bridge_account_rollup_id(from, cache)
                .await
                .context("failed to get bridge account rollup id")?
                .is_none(),
            "cannot transfer out of bridge account; BridgeUnlock must be used",
        );
        println!(
            "IN check_and_execute: get_bridge_account_rollup_id: {}",
            s.elapsed().as_secs_f32()
        );
        s = std::time::Instant::now();

        // println!("START TRANSFER");
        check_transfer(self, from, &state, cache).await?;
        println!(
            "IN check_and_execute: check_xfer: {}",
            s.elapsed().as_secs_f32()
        );
        s = std::time::Instant::now();
        execute_transfer(self, from, state, cache).await?;
        println!(
            "IN check_and_execute: exec_xfer: {}",
            s.elapsed().as_secs_f32()
        );
        println!("DONE TRANSFER: {}", s1.elapsed().as_secs_f32());

        Ok(())
    }
}

pub(crate) async fn execute_transfer<S: StateWrite>(
    action: &TransferAction,
    from: [u8; ADDRESS_LEN],
    mut state: S,
    cache: &Cache,
) -> anyhow::Result<()> {
    // let mut s = std::time::Instant::now();
    let fee = state
        .get_transfer_base_fee(cache)
        .await
        .context("failed to get transfer base fee")?;
    // println!(
    //     "execute_transfer: get_transfer_base_fee: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();
    state
        .get_and_increase_block_fees(&action.fee_asset, fee, TransferAction::full_name(), cache)
        .await
        .context("failed to add to block fees")?;
    // println!(
    //     "execute_transfer: get_and_increase_block_fees: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();

    // if fee payment asset is same asset as transfer asset, deduct fee
    // from same balance as asset transferred
    if action.asset.to_ibc_prefixed() == action.fee_asset.to_ibc_prefixed() {
        // check_stateful should have already checked this arithmetic
        let payment_amount = action
            .amount
            .checked_add(fee)
            .expect("transfer amount plus fee should not overflow");
        // println!(
        //     "execute_transfer: payment_amount: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();

        state
            .decrease_balance(from, &action.asset, payment_amount, cache)
            .await
            .context("failed decreasing `from` account balance")?;
        // println!(
        //     "execute_transfer: decrease_balance AAA: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();
        state
            .increase_balance(action.to, &action.asset, action.amount, cache)
            .await
            .context("failed increasing `to` account balance")?;
        // println!(
        //     "execute_transfer: increase_balance AAA: {}",
        //     s.elapsed().as_secs_f32()
        // );
    } else {
        // otherwise, just transfer the transfer asset and deduct fee from fee asset balance
        // later
        state
            .decrease_balance(from, &action.asset, action.amount, cache)
            .await
            .context("failed decreasing `from` account balance")?;
        // println!(
        //     "execute_transfer: decrease_balance BBB: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();
        state
            .increase_balance(action.to, &action.asset, action.amount, cache)
            .await
            .context("failed increasing `to` account balance")?;
        // println!(
        //     "execute_transfer: increase_balance BBB: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();

        // deduct fee from fee asset balance
        state
            .decrease_balance(from, &action.fee_asset, fee, cache)
            .await
            .context("failed decreasing `from` account balance for fee payment")?;
        // println!(
        //     "execute_transfer: decrease_balance CCC: {}",
        //     s.elapsed().as_secs_f32()
        // );
    }
    Ok(())
}

pub(crate) async fn check_transfer<S, TAddress>(
    action: &TransferAction,
    from: TAddress,
    state: &S,
    cache: &Cache,
) -> Result<()>
where
    S: StateRead,
    TAddress: AddressBytes,
{
    // let mut s = std::time::Instant::now();
    state.ensure_base_prefix(&action.to, cache).await.context(
        "failed ensuring that the destination address matches the permitted base prefix",
    )?;
    // println!(
    //     "check_transfer: ensure_base_prefix: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();
    ensure!(
        state
            .is_allowed_fee_asset(&action.fee_asset, cache)
            .await
            .context("failed to check allowed fee assets in state")?,
        "invalid fee asset",
    );
    // println!(
    //     "check_transfer: is_allowed_fee_asset: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();

    let fee = state
        .get_transfer_base_fee(cache)
        .await
        .context("failed to get transfer base fee")?;
    // println!(
    //     "check_transfer: get_transfer_base_fee: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();
    let transfer_asset = action.asset.clone();

    let from_fee_balance = state
        .get_account_balance(&from, &action.fee_asset, cache)
        .await
        .context("failed getting `from` account balance for fee payment")?;
    // println!(
    //     "check_transfer: get_account_balance 111: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();

    // if fee asset is same as transfer asset, ensure accounts has enough funds
    // to cover both the fee and the amount transferred
    let a = action.fee_asset.to_ibc_prefixed();
    let b = transfer_asset.to_ibc_prefixed();
    // println!(
    //     "check_transfer: to_ibc_prefixed * 2: {}",
    //     s.elapsed().as_secs_f32()
    // );
    // s = std::time::Instant::now();
    if a == b {
        let payment_amount = action
            .amount
            .checked_add(fee)
            .context("transfer amount plus fee overflowed")?;
        // println!(
        //     "check_transfer: payment_amount: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();

        ensure!(
            from_fee_balance >= payment_amount,
            "insufficient funds for transfer and fee payment"
        );
        // println!(
        //     "check_transfer: check balance AAA: {}",
        //     s.elapsed().as_secs_f32()
        // );
    } else {
        // otherwise, check the fee asset account has enough to cover the fees,
        // and the transfer asset account has enough to cover the transfer
        ensure!(
            from_fee_balance >= fee,
            "insufficient funds for fee payment"
        );
        // println!(
        //     "check_transfer: check balance BBB: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();

        let from_transfer_balance = state
            .get_account_balance(from, transfer_asset, cache)
            .await
            .context("failed to get account balance in transfer check")?;
        // println!(
        //     "check_transfer: get_account_balance 222: {}",
        //     s.elapsed().as_secs_f32()
        // );
        // s = std::time::Instant::now();
        ensure!(
            from_transfer_balance >= action.amount,
            "insufficient funds for transfer"
        );
        // println!(
        //     "check_transfer: check balance CCC: {}",
        //     s.elapsed().as_secs_f32()
        // );
    }

    Ok(())
}
