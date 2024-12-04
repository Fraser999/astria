use std::time::Duration;

use prost::Message;
use tendermint::{
    abci::types::{
        BlockSignatureInfo,
        ExtendedCommitInfo,
        ExtendedVoteInfo,
        Validator,
    },
    block::{
        BlockIdFlag,
        Round,
    },
    vote::Power,
};

use super::utils::{
    calculate_prices_from_vote_extensions,
    test_helpers::{
        get_id_to_currency_pair_mapping,
        oracle_vote_extension,
    },
};

/// The max time for any benchmark.
const MAX_TIME: Duration = Duration::from_secs(120);

#[divan::bench(max_time = MAX_TIME)]
fn calculate_connect_prices(bencher: divan::Bencher) {
    const CURRENCY_PAIR_COUNT: usize = 2000;
    const VALIDATOR_COUNT: usize = 100;

    let id_to_currency_pairs = get_id_to_currency_pair_mapping(CURRENCY_PAIR_COUNT);
    let validator = Validator {
        address: [1; 20],
        power: Power::from(10_u8),
    };
    let sig_info = BlockSignatureInfo::Flag(BlockIdFlag::Commit);
    bencher
        .with_inputs(|| {
            let mut votes = Vec::with_capacity(VALIDATOR_COUNT);
            for validator_index in 0..VALIDATOR_COUNT {
                let first_price = 100_000_000_000_000_000_u128
                    .checked_add(validator_index as u128)
                    .unwrap();
                let last_price = first_price
                    .checked_add(CURRENCY_PAIR_COUNT as u128)
                    .unwrap();
                let vote_extension = oracle_vote_extension(first_price..last_price);
                votes.push(ExtendedVoteInfo {
                    validator: validator.clone(),
                    sig_info,
                    vote_extension: vote_extension.into_raw().encode_to_vec().into(),
                    extension_signature: None,
                });
            }

            ExtendedCommitInfo {
                round: Round::default(),
                votes,
            }
        })
        .bench_values(move |extended_commit_info| {
            calculate_prices_from_vote_extensions(extended_commit_info, &id_to_currency_pairs)
        });
}
