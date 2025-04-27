use astria_core::{
    primitive::v1::{
        asset::IbcPrefixed,
        ADDRESS_LEN,
    },
    protocol::transaction::v1::action::MarketsChange,
};
use astria_eyre::eyre::{
    bail,
    ensure,
    eyre,
    OptionExt as _,
    Result,
    WrapErr as _,
};
use cnidarium::{
    StateRead,
    StateWrite,
};
use tracing::{
    instrument,
    Level,
};

use super::{
    AssetTransfer,
    TransactionSignerAddressBytes,
};
use crate::{
    app::StateReadExt,
    authority::StateReadExt as _,
    oracles::price_feed::market_map::state_ext::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedMarketsChange {
    action: MarketsChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedMarketsChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: MarketsChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let checked_action = Self {
            action,
            tx_signer: tx_signer.into(),
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Ensure the tx signer is the current sudo address.
        let sudo_address = state
            .get_sudo_address()
            .await
            .wrap_err("failed to read sudo address from storage")?;
        ensure!(
            &sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change markets",
        );
        Ok(())
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;

        let mut market_map = state
            .get_market_map()
            .await
            .wrap_err("failed to read market map from storage")?
            .ok_or_eyre("market map not found in storage")?;
        match &self.action {
            MarketsChange::Creation(create_markets) => {
                for market in create_markets {
                    let ticker_key = market.ticker.currency_pair.to_string();
                    if market_map.markets.contains_key(&ticker_key) {
                        bail!("market for ticker {ticker_key} already exists");
                    }
                    market_map.markets.insert(ticker_key, market.clone());
                }
            }
            MarketsChange::Removal(remove_markets) => {
                for key in remove_markets {
                    market_map
                        .markets
                        .shift_remove(&key.ticker.currency_pair.to_string());
                }
            }
            MarketsChange::Update(update_markets) => {
                if market_map.markets.is_empty() {
                    bail!("market map is empty");
                }
                for market in update_markets {
                    let ticker_key = market.ticker.currency_pair.to_string();
                    *market_map.markets.get_mut(&ticker_key).ok_or_else(|| {
                        eyre!("market for ticker {ticker_key} not found in market map")
                    })? = market.clone();
                }
            }
        };

        state
            .put_market_map(market_map)
            .wrap_err("failed to write market map to storage")?;

        // update the last updated height for the market map
        let block_height = state
            .get_block_height()
            .await
            .wrap_err("failed to read block height from storage")?;
        state
            .put_market_map_last_updated_height(block_height)
            .wrap_err("failed to write latest market map height to storage")?;
        Ok(())
    }

    pub(super) fn action(&self) -> &MarketsChange {
        &self.action
    }
}

impl AssetTransfer for CheckedMarketsChange {
    fn transfer_asset_and_amount(&self) -> Option<(IbcPrefixed, u128)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use astria_core::{
        oracles::price_feed::types::v2::CurrencyPairId,
        protocol::transaction::v1::action::SudoAddressChange,
    };

    use super::*;
    use crate::{
        benchmark_and_test_utils::astria_address,
        checked_actions::CheckedSudoAddressChange,
        test_utils::{
            assert_error_contains,
            Fixture,
            SUDO_ADDRESS_BYTES,
        },
    };

    // #[tokio::test]
    // async fn should_fail_construction_if_signer_is_not_sudo_address() {
    //     let fixture = Fixture::default_initialized().await;
    //
    //     let tx_signer = [2_u8; ADDRESS_LEN];
    //     assert_ne!(*SUDO_ADDRESS_BYTES, tx_signer);
    //
    //     let addition = new_addition(Some("BTC/USD"));
    //     let err = fixture
    //         .new_checked_action(addition, tx_signer)
    //         .await
    //         .unwrap_err();
    //     assert_error_contains(
    //         &err,
    //         "transaction signer not authorized to change currency pairs",
    //     );
    //
    //     let removal = new_removal(Some("BTC/USD"));
    //     let err = fixture
    //         .new_checked_action(removal, tx_signer)
    //         .await
    //         .unwrap_err();
    //     assert_error_contains(
    //         &err,
    //         "transaction signer not authorized to change currency pairs",
    //     );
    // }
    //
    // #[tokio::test]
    // async fn should_fail_execution_if_signer_is_not_sudo_address() {
    //     let mut fixture = Fixture::default_initialized().await;
    //
    //     // Construct the addition and removal checked actions while the sudo address is still the
    //     // tx signer so construction succeeds.
    //     let addition_action = new_addition(Some("BTC/USD"));
    //     let checked_addition_action: CheckedCurrencyPairsChange = fixture
    //         .new_checked_action(addition_action, *SUDO_ADDRESS_BYTES)
    //         .await
    //         .unwrap()
    //         .into();
    //
    //     let removal_action = new_removal(Some("BTC/USD"));
    //     let checked_removal_action: CheckedCurrencyPairsChange = fixture
    //         .new_checked_action(removal_action, *SUDO_ADDRESS_BYTES)
    //         .await
    //         .unwrap()
    //         .into();
    //
    //     // Change the sudo address to something other than the tx signer.
    //     let sudo_address_change = SudoAddressChange {
    //         new_address: astria_address(&[2; ADDRESS_LEN]),
    //     };
    //     let checked_sudo_address_change: CheckedSudoAddressChange = fixture
    //         .new_checked_action(sudo_address_change, *SUDO_ADDRESS_BYTES)
    //         .await
    //         .unwrap()
    //         .into();
    //     checked_sudo_address_change
    //         .execute(fixture.state_mut())
    //         .await
    //         .unwrap();
    //     let new_sudo_address = fixture.state().get_sudo_address().await.unwrap();
    //     assert_ne!(*SUDO_ADDRESS_BYTES, new_sudo_address);
    //
    //     // Try to execute the two checked actions now - should fail due to signer no longer being
    //     // authorized.
    //     let err = checked_addition_action
    //         .execute(fixture.state_mut())
    //         .await
    //         .unwrap_err();
    //     assert_error_contains(
    //         &err,
    //         "transaction signer not authorized to change currency pairs",
    //     );
    //
    //     let err = checked_removal_action
    //         .execute(fixture.state_mut())
    //         .await
    //         .unwrap_err();
    //     assert_error_contains(
    //         &err,
    //         "transaction signer not authorized to change currency pairs",
    //     );
    // }
    //
    // #[tokio::test]
    // async fn should_execute_addition() {
    //     let mut fixture = Fixture::default_initialized().await;
    //
    //     // Ensure providing duplicate pairs succeeds.
    //     let action = new_addition(["BTC/USD", "ETH/USD", "BTC/USD"]);
    //     let checked_action: CheckedCurrencyPairsChange = fixture
    //         .new_checked_action(action.clone(), *SUDO_ADDRESS_BYTES)
    //         .await
    //         .unwrap()
    //         .into();
    //
    //     checked_action.execute(fixture.state_mut()).await.unwrap();
    //
    //     let pairs = pairs(action);
    //     assert_eq!(
    //         fixture
    //             .state()
    //             .get_currency_pair_state(&pairs[0])
    //             .await
    //             .unwrap()
    //             .unwrap(),
    //         CurrencyPairState {
    //             price: None,
    //             nonce: CurrencyPairNonce::new(0),
    //             id: CurrencyPairId::new(0),
    //         }
    //     );
    //     assert_eq!(
    //         fixture
    //             .state()
    //             .get_currency_pair_state(&pairs[1])
    //             .await
    //             .unwrap()
    //             .unwrap(),
    //         CurrencyPairState {
    //             price: None,
    //             nonce: CurrencyPairNonce::new(0),
    //             id: CurrencyPairId::new(1),
    //         }
    //     );
    //     assert_eq!(
    //         fixture.state().get_next_currency_pair_id().await.unwrap(),
    //         CurrencyPairId::new(2)
    //     );
    //     assert_eq!(fixture.state().get_num_currency_pairs().await.unwrap(), 2);
    // }
    //
    // #[tokio::test]
    // async fn should_execute_removal() {
    //     let mut fixture = Fixture::default_initialized().await;
    //
    //     // Add two currency pairs.
    //     let addition_action = new_addition(["BTC/USD", "TIA/USD"]);
    //     let checked_addition_action: CheckedCurrencyPairsChange = fixture
    //         .new_checked_action(addition_action, *SUDO_ADDRESS_BYTES)
    //         .await
    //         .unwrap()
    //         .into();
    //     checked_addition_action
    //         .execute(fixture.state_mut())
    //         .await
    //         .unwrap();
    //
    //     // Ensure removing duplicate pairs succeeds, and removing a non-existent pair succeeds.
    //     let action = new_removal(["BTC/USD", "ETH/USD", "BTC/USD"]);
    //     let checked_action: CheckedCurrencyPairsChange = fixture
    //         .new_checked_action(action.clone(), *SUDO_ADDRESS_BYTES)
    //         .await
    //         .unwrap()
    //         .into();
    //
    //     checked_action.execute(fixture.state_mut()).await.unwrap();
    //
    //     assert!(fixture
    //         .state()
    //         .get_currency_pair_state(&"BTC/USD".parse::<CurrencyPair>().unwrap())
    //         .await
    //         .unwrap()
    //         .is_none());
    //     assert!(fixture
    //         .state()
    //         .get_currency_pair_state(&"ETH/USD".parse::<CurrencyPair>().unwrap())
    //         .await
    //         .unwrap()
    //         .is_none());
    //     assert_eq!(
    //         fixture
    //             .state()
    //             .get_currency_pair_state(&"TIA/USD".parse::<CurrencyPair>().unwrap())
    //             .await
    //             .unwrap()
    //             .unwrap(),
    //         CurrencyPairState {
    //             price: None,
    //             nonce: CurrencyPairNonce::new(0),
    //             id: CurrencyPairId::new(1),
    //         }
    //     );
    //     assert_eq!(
    //         fixture.state().get_next_currency_pair_id().await.unwrap(),
    //         CurrencyPairId::new(2)
    //     );
    //     assert_eq!(fixture.state().get_num_currency_pairs().await.unwrap(), 1);
    // }
}
