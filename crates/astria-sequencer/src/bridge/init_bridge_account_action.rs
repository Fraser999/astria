use anyhow::{
    bail,
    ensure,
    Context as _,
    Result,
};
use astria_core::{
    primitive::v1::Address,
    protocol::transaction::v1alpha1::action::InitBridgeAccountAction,
};
use cnidarium::StateWrite;

use crate::{
    accounts::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    address::StateReadExt as _,
    app::ActionHandler,
    assets::StateReadExt as _,
    bridge::state_ext::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    cache::Cache,
    // transaction::StateReadExt as _,
};

#[async_trait::async_trait]
impl ActionHandler for InitBridgeAccountAction {
    type CheckStatelessContext = ();

    async fn check_stateless(&self, _context: Self::CheckStatelessContext) -> Result<()> {
        Ok(())
    }

    async fn check_and_execute<S: StateWrite>(
        &self,
        from: [u8; 20],
        mut state: S,
        cache: &Cache,
    ) -> Result<()> {
        // let from = state
        //     .get_current_source()
        //     .expect("transaction source must be present in state when executing an action")
        //     .address_bytes();
        if let Some(withdrawer_address) = &self.withdrawer_address {
            state
                .ensure_base_prefix(withdrawer_address, cache)
                .await
                .context("failed check for base prefix of withdrawer address")?;
        }
        if let Some(sudo_address) = &self.sudo_address {
            state
                .ensure_base_prefix(sudo_address, cache)
                .await
                .context("failed check for base prefix of sudo address")?;
        }

        ensure!(
            state.is_allowed_fee_asset(&self.fee_asset, cache).await?,
            "invalid fee asset",
        );

        let fee = state
            .get_init_bridge_account_base_fee()
            .await
            .context("failed to get base fee for initializing bridge account")?;

        // this prevents the address from being registered as a bridge account
        // if it's been previously initialized as a bridge account.
        //
        // however, there is no prevention of initializing an account as a bridge
        // account that's already been used as a normal EOA.
        //
        // the implication is that the account might already have a balance, nonce, etc.
        // before being converted into a bridge account.
        //
        // after the account becomes a bridge account, it can no longer receive funds
        // via `TransferAction`, only via `BridgeLockAction`.
        if state
            .get_bridge_account_rollup_id(from, cache)
            .await
            .context("failed getting rollup ID of bridge account")?
            .is_some()
        {
            bail!("bridge account already exists");
        }

        let balance = state
            .get_account_balance(from, &self.fee_asset, cache)
            .await
            .context("failed getting `from` account balance for fee payment")?;

        ensure!(
            balance >= fee,
            "insufficient funds for bridge account initialization",
        );

        state.put_bridge_account_rollup_id(from, &self.rollup_id);
        state
            .put_bridge_account_ibc_asset(from, &self.asset)
            .context("failed to put asset ID")?;
        state.put_bridge_account_sudo_address(from, self.sudo_address.map_or(from, Address::bytes));
        state.put_bridge_account_withdrawer_address(
            from,
            self.withdrawer_address.map_or(from, Address::bytes),
        );

        state
            .decrease_balance(from, &self.fee_asset, fee, cache)
            .await
            .context("failed to deduct fee from account balance")?;
        Ok(())
    }
}
