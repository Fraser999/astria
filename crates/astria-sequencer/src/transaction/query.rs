use std::str::FromStr;

use astria_core::{
    generated::protocol::transaction::v1alpha1::UnsignedTransaction as RawUnsignedTransaction,
    primitive::v1::asset,
    protocol::{
        abci::AbciErrorCode,
        transaction::v1alpha1::UnsignedTransaction,
    },
    sequencer::Fees,
};
use cnidarium::Storage;
use prost::Message as _;
use tendermint::abci::{
    request,
    response,
};

use crate::{
    assets::StateReadExt as _,
    immutable_data::ImmutableData,
    state_ext::StateReadExt as _,
    transaction::checks::get_fees_for_transaction,
};

pub(crate) async fn transaction_fee_request(
    storage: Storage,
    request: request::Query,
    _params: Vec<(String, String)>,
) -> response::Query {
    use astria_core::protocol::transaction::v1alpha1::TransactionFeeResponse;

    let tx = match preprocess_request(&request) {
        Ok(tx) => tx,
        Err(err_rsp) => return err_rsp,
    };

    // use latest snapshot, as this is a query for a transaction fee
    let snapshot = storage.latest_snapshot();
    let height = match snapshot.get_block_height().await {
        Ok(height) => height,
        Err(err) => {
            return response::Query {
                code: AbciErrorCode::INTERNAL_ERROR.into(),
                info: AbciErrorCode::INTERNAL_ERROR.to_string(),
                log: format!("failed getting block height: {err:#}"),
                ..response::Query::default()
            };
        }
    };

    let immutable_data = ImmutableData {
        base_prefix: "2".to_string(),
        fees: Fees {
            transfer_base_fee: 0,
            sequence_base_fee: 0,
            sequence_byte_cost_multiplier: 0,
            init_bridge_account_base_fee: 0,
            bridge_lock_byte_cost_multiplier: 0,
            bridge_sudo_change_fee: 0,
            ics20_withdrawal_base_fee: 0,
        },
        native_asset: asset::TracePrefixed::from_str("f").unwrap(),
        chain_id: tendermint::chain::Id::from_str("s").unwrap(),
        authority_sudo_address: [0; 20],
        ibc_sudo_address: [0; 20],
    };
    let fees_with_ibc_denoms = match get_fees_for_transaction(&tx, &snapshot, &immutable_data) {
        Ok(fees) => fees,
        Err(err) => {
            return response::Query {
                code: AbciErrorCode::INTERNAL_ERROR.into(),
                info: AbciErrorCode::INTERNAL_ERROR.to_string(),
                log: format!("failed calculating fees for provided transaction: {err:#}"),
                ..response::Query::default()
            };
        }
    };

    let mut fees = Vec::with_capacity(fees_with_ibc_denoms.len());
    for (ibc_denom, value) in fees_with_ibc_denoms {
        let trace_denom = match snapshot.map_ibc_to_trace_prefixed_asset(ibc_denom).await {
            Ok(Some(trace_denom)) => trace_denom,
            Ok(None) => {
                return response::Query {
                    code: AbciErrorCode::INTERNAL_ERROR.into(),
                    info: AbciErrorCode::INTERNAL_ERROR.to_string(),
                    log: format!(
                        "failed mapping ibc denom to trace denom: {ibc_denom}; asset does not \
                         exist in state"
                    ),
                    ..response::Query::default()
                };
            }
            Err(err) => {
                return response::Query {
                    code: AbciErrorCode::INTERNAL_ERROR.into(),
                    info: AbciErrorCode::INTERNAL_ERROR.to_string(),
                    log: format!("failed mapping ibc denom to trace denom: {err:#}"),
                    ..response::Query::default()
                };
            }
        };
        fees.push((trace_denom.into(), value));
    }

    let resp = TransactionFeeResponse {
        height,
        fees,
    };

    let payload = resp.into_raw().encode_to_vec().into();

    let height = tendermint::block::Height::try_from(height).expect("height must fit into an i64");
    response::Query {
        code: 0.into(),
        key: request.path.into_bytes().into(),
        value: payload,
        height,
        ..response::Query::default()
    }
}

fn preprocess_request(request: &request::Query) -> Result<UnsignedTransaction, response::Query> {
    let tx = match RawUnsignedTransaction::decode(&*request.data) {
        Ok(tx) => tx,
        Err(err) => {
            return Err(response::Query {
                code: AbciErrorCode::BAD_REQUEST.into(),
                info: AbciErrorCode::BAD_REQUEST.to_string(),
                log: format!("failed to decode request data to unsigned transaction: {err:#}"),
                ..response::Query::default()
            });
        }
    };

    let tx = match UnsignedTransaction::try_from_raw(tx) {
        Ok(tx) => tx,
        Err(err) => {
            return Err(response::Query {
                code: AbciErrorCode::BAD_REQUEST.into(),
                info: AbciErrorCode::BAD_REQUEST.to_string(),
                log: format!(
                    "failed to convert raw proto unsigned transaction to native unsigned \
                     transaction: {err:#}"
                ),
                ..response::Query::default()
            });
        }
    };

    Ok(tx)
}
