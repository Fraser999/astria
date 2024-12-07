use astria_core::upgrades::v1::Upgrades;
use astria_eyre::eyre::Result;
use cnidarium::{
    Snapshot,
};
pub(crate) use upgrade::Upgrade;

use crate::app::ShouldShutDown;

mod state_ext;
pub(crate) mod storage;
mod upgrade;

fn upgrades_iter(upgrades: &Upgrades) -> impl Iterator<Item=&'_ dyn Upgrade> {
    let Upgrades {
        upgrade_1,
    } = upgrades;

    upgrade_1.as_ref().into_iter()
}

/// Returns `ShouldShutDown::ShutDownForUpgrade` if any scheduled upgrade is due to be applied
/// during the next block and this binary is not hard-coded to apply that upgrade.  Otherwise,
/// returns `ShouldShutDown::ContinueRunning`.
pub(crate) async fn should_shut_down(
    upgrades: &Upgrades,
    snapshot: &Snapshot,
) -> Result<ShouldShutDown> {
    for upgrade in upgrades_iter(upgrades) {
        match upgrade.should_shut_down(snapshot).await? {
            res @ ShouldShutDown::ShutDownForUpgrade {
                ..
            } => return Ok(res),
            ShouldShutDown::ContinueRunning => {}
        }
    }
    Ok(ShouldShutDown::ContinueRunning)
}

/// Verifies that all historical upgrades have been applied.
///
/// Returns an error if any has not been applied.
pub(crate) async fn ensure_historical_upgrades_applied(
    upgrades: &Upgrades,
    snapshot: &Snapshot,
) -> Result<()> {
    for upgrade in upgrades_iter(upgrades) {
        upgrade.ensure_historical_upgrades_applied(snapshot)?
    }
    Ok(())
}

#[test]
fn toodoo() {
    todo!("check for unwraps in new code");
    todo!("add config env var and update charts");
    todo!("tests for new code");
}

// #[test]
// fn dump_upgrade_file_example() {
//     use astria_core::{
//         generated::connect::{
//             marketmap::v2::*,
//             oracle::v2::*,
//             types::v2::*,
//         },
//         primitive::v1::Address,
//     };
//
//     const ASTRIA_ADDRESS_PREFIX: &str = "astria";
//
//     fn alice() -> Address {
//         Address::builder()
//             .prefix(ASTRIA_ADDRESS_PREFIX)
//             .slice(hex::decode("1c0c490f1b5528d8173c5de46d131160e4b2c0c3").unwrap())
//             .try_build()
//             .unwrap()
//     }
//
//     fn bob() -> Address {
//         Address::builder()
//             .prefix(ASTRIA_ADDRESS_PREFIX)
//             .slice(hex::decode("34fec43c7fcab9aef3b3cf8aba855e41ee69ca3a").unwrap())
//             .try_build()
//             .unwrap()
//     }
//
//     fn genesis_state_markets() -> MarketMap {
//         use maplit::{
//             btreemap,
//             convert_args,
//         };
//         let markets = convert_args!(btreemap!(
//             "BTC/USD" => Market {
//                 ticker: Some(Ticker {
//                     currency_pair: Some(CurrencyPair {
//                         base: "BTC".to_string(),
//                         quote: "USD".to_string(),
//                     }),
//                     decimals: 8,
//                     min_provider_count: 1,
//                     enabled: true,
//                     metadata_json: String::new(),
//                 }),
//                 provider_configs: vec![ProviderConfig {
//                     name: "coingecko_api".to_string(),
//                     off_chain_ticker: "bitcoin/usd".to_string(),
//                     normalize_by_pair: None,
//                     invert: false,
//                     metadata_json: String::new(),
//                 }],
//             },
//             "ETH/USD" => Market {
//                 ticker: Some(Ticker {
//                     currency_pair: Some(CurrencyPair {
//                         base: "ETH".to_string(),
//                         quote: "USD".to_string(),
//                     }),
//                     decimals: 8,
//                     min_provider_count: 1,
//                     enabled: true,
//                     metadata_json: String::new(),
//                 }),
//                 provider_configs: vec![ProviderConfig {
//                     name: "coingecko_api".to_string(),
//                     off_chain_ticker: "ethereum/usd".to_string(),
//                     normalize_by_pair: None,
//                     invert: false,
//                     metadata_json: String::new(),
//                 }],
//             },
//         ));
//         MarketMap {
//             markets,
//         }
//     }
//
//     let connect = astria_core::generated::protocol::genesis::v1::ConnectGenesis {
//         market_map: Some(
//             astria_core::generated::connect::marketmap::v2::GenesisState {
//                 market_map: Some(genesis_state_markets()),
//                 last_updated: 0,
//                 params: Some(Params {
//                     market_authorities: vec![alice().to_string(), bob().to_string()],
//                     admin: alice().to_string(),
//                 }),
//             },
//         ),
//         oracle: Some(astria_core::generated::connect::oracle::v2::GenesisState {
//             currency_pair_genesis: vec![
//                 CurrencyPairGenesis {
//                     id: 0,
//                     nonce: 0,
//                     currency_pair_price: Some(QuotePrice {
//                         price: 5_834_065_777_u128.to_string(),
//                         block_height: 0,
//                         block_timestamp: Some(pbjson_types::Timestamp {
//                             seconds: 1_720_122_395,
//                             nanos: 0,
//                         }),
//                     }),
//                     currency_pair: Some(CurrencyPair {
//                         base: "BTC".to_string(),
//                         quote: "USD".to_string(),
//                     }),
//                 },
//                 CurrencyPairGenesis {
//                     id: 1,
//                     nonce: 0,
//                     currency_pair_price: Some(QuotePrice {
//                         price: 3_138_872_234_u128.to_string(),
//                         block_height: 0,
//                         block_timestamp: Some(pbjson_types::Timestamp {
//                             seconds: 1_720_122_395,
//                             nanos: 0,
//                         }),
//                     }),
//                     currency_pair: Some(CurrencyPair {
//                         base: "ETH".to_string(),
//                         quote: "USD".to_string(),
//                     }),
//                 },
//             ],
//             next_id: 2,
//         }),
//     };
//
//     let u = Upgrades {
//         file_hash: [0; 32],
//         connect_oracle: Some(ConnectOracleUpgrade {
//             activation_height: 100,
//             genesis: Arc::new(connect.try_into().unwrap()),
//         }),
//         validator_update_action: Some(ValidatorUpdateActionUpgrade {
//             activation_height: 200,
//         }),
//     };
//     println!("{}", serde_json::to_string_pretty(&u).unwrap());
// }
