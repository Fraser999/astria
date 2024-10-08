use astria_core::protocol::transaction::v1alpha1::action::Sequence;
use astria_eyre::eyre::{
    ensure,
    OptionExt as _,
    Result,
    WrapErr as _,
};
use cnidarium::StateWrite;

use crate::{
    accounts::{
        StateReadExt as _,
        StateWriteExt as _,
    },
    app::ActionHandler,
    assets::{
        StateReadExt,
        StateWriteExt,
    },
    sequence,
    transaction::StateReadExt as _,
};

#[async_trait::async_trait]
impl ActionHandler for Sequence {
    async fn check_stateless(&self) -> Result<()> {
        // TODO: do we want to place a maximum on the size of the data?
        // https://github.com/astriaorg/astria/issues/222
        ensure!(
            !self.data.is_empty(),
            "cannot have empty data for sequence action"
        );
        Ok(())
    }

    async fn check_and_execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        let from = state
            .get_transaction_context()
            .expect("transaction source must be present in state when executing an action")
            .address_bytes();

        ensure!(
            state
                .is_allowed_fee_asset(&self.fee_asset)
                .await
                .wrap_err("failed accessing state to check if fee is allowed")?,
            "invalid fee asset",
        );

        let curr_balance = state
            .get_account_balance(&from, &self.fee_asset)
            .await
            .wrap_err("failed getting `from` account balance for fee payment")?;
        let fee = calculate_fee_from_state(&self.data, &state)
            .await
            .wrap_err("calculated fee overflows u128")?;
        ensure!(curr_balance >= fee, "insufficient funds");

        state
            .get_and_increase_block_fees::<Self, _>(&self.fee_asset, fee)
            .await
            .wrap_err("failed to add to block fees")?;
        state
            .decrease_balance(&from, &self.fee_asset, fee)
            .await
            .wrap_err("failed updating `from` account balance")?;
        Ok(())
    }
}

/// Calculates the fee for a sequence `Action` based on the length of the `data`.
pub(crate) async fn calculate_fee_from_state<S: sequence::StateReadExt>(
    data: &[u8],
    state: &S,
) -> Result<u128> {
    let base_fee = state
        .get_sequence_action_base_fee()
        .await
        .wrap_err("failed to get base fee")?;
    let fee_per_byte = state
        .get_sequence_action_byte_cost_multiplier()
        .await
        .wrap_err("failed to get fee per byte")?;
    calculate_fee(data, fee_per_byte, base_fee).ok_or_eyre("calculated fee overflows u128")
}

/// Calculates the fee for a sequence `Action` based on the length of the `data`.
/// Returns `None` if the fee overflows `u128`.
fn calculate_fee(data: &[u8], fee_per_byte: u128, base_fee: u128) -> Option<u128> {
    base_fee.checked_add(
        fee_per_byte.checked_mul(
            data.len()
                .try_into()
                .expect("a usize should always convert to a u128"),
        )?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_fee_ok() {
        assert_eq!(calculate_fee(&[], 1, 0), Some(0));
        assert_eq!(calculate_fee(&[0], 1, 0), Some(1));
        assert_eq!(calculate_fee(&[0u8; 10], 1, 0), Some(10));
        assert_eq!(calculate_fee(&[0u8; 10], 1, 100), Some(110));
    }
}
