use std::future::ready;

use astria_core::{
    generated::astria::protocol::transaction::v1::TransactionBody as RawBody,
    primitive::v1::asset::{
        self,
        Denom,
    },
    protocol::{
        abci::AbciErrorCode,
        asset::v1::AllowedFeeAssetsResponse,
        fees::v1::FeeComponents,
        transaction::v1::{
            action::{
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
                RecoverIbcClient,
                RollupDataSubmission,
                SudoAddressChange,
                Transfer,
                ValidatorUpdate,
            },
            TransactionBody,
        },
    },
    Protobuf as _,
};
use astria_eyre::eyre::{
    self,
    OptionExt as _,
    WrapErr as _,
};
use cnidarium::{
    StateRead,
    Storage,
};
use futures::{
    FutureExt as _,
    StreamExt as _,
};
use penumbra_ibc::IbcRelay;
use prost::{
    Message as _,
    Name as _,
};
use tendermint::abci::{
    request,
    response,
    Code,
};
use tokio::{
    join,
    try_join,
};
use tracing::{
    instrument,
    warn,
};

use crate::{
    app::StateReadExt as _,
    assets::StateReadExt as _,
    checked_actions::{
        utils::total_fees,
        ActionRef,
    },
    fees::StateReadExt as _,
};

#[instrument(skip_all, fields(%asset))]
async fn find_trace_prefixed_or_return_ibc<S: StateRead>(
    state: S,
    asset: asset::IbcPrefixed,
) -> asset::Denom {
    state
        .map_ibc_to_trace_prefixed_asset(&asset)
        .await
        .wrap_err("failed to get ibc asset denom")
        .and_then(|maybe_asset| {
            maybe_asset.ok_or_eyre("ibc-prefixed asset did not have an entry in state")
        })
        .map_or_else(|_| asset.into(), Into::into)
}

#[instrument(skip_all)]
async fn get_allowed_fee_assets<S: StateRead>(state: &S) -> Vec<Denom> {
    let stream = state
        .allowed_fee_assets()
        .filter_map(|asset| {
            ready(
                asset
                    .inspect_err(|error| warn!(%error, "encountered issue reading allowed assets"))
                    .ok(),
            )
        })
        .map(|asset| find_trace_prefixed_or_return_ibc(state, asset))
        .buffered(16);
    stream.collect::<Vec<_>>().await
}

#[instrument(skip_all)]
pub(crate) async fn allowed_fee_assets_request(
    storage: Storage,
    request: request::Query,
    _params: Vec<(String, String)>,
) -> response::Query {
    // get last snapshot
    let snapshot = storage.latest_snapshot();

    let height = async {
        snapshot
            .get_block_height()
            .await
            .wrap_err("failed getting block height")
    };
    let fee_assets = get_allowed_fee_assets(&snapshot).map(Ok);
    let (height, fee_assets) = match try_join!(height, fee_assets) {
        Ok(vals) => vals,
        Err(err) => {
            return response::Query {
                code: Code::Err(AbciErrorCode::INTERNAL_ERROR.value()),
                info: AbciErrorCode::INTERNAL_ERROR.info(),
                log: format!("{err:#}"),
                ..response::Query::default()
            };
        }
    };

    let payload = AllowedFeeAssetsResponse {
        height,
        fee_assets: fee_assets.into_iter().map(Into::into).collect(),
    }
    .into_raw()
    .encode_to_vec()
    .into();

    let height = tendermint::block::Height::try_from(height).expect("height must fit into an i64");
    response::Query {
        code: tendermint::abci::Code::Ok,
        key: request.path.into_bytes().into(),
        value: payload,
        height,
        ..response::Query::default()
    }
}

pub(crate) async fn components(
    storage: Storage,
    request: request::Query,
    _params: Vec<(String, String)>,
) -> response::Query {
    let snapshot = storage.latest_snapshot();

    let height = async {
        snapshot
            .get_block_height()
            .await
            .wrap_err("failed getting block height")
    };
    let fee_components = get_all_fee_components(&snapshot).map(Ok);
    let (height, fee_components) = match try_join!(height, fee_components) {
        Ok(vals) => vals,
        Err(err) => {
            return response::Query {
                code: Code::Err(AbciErrorCode::INTERNAL_ERROR.value()),
                info: AbciErrorCode::INTERNAL_ERROR.info(),
                log: format!("{err:#}"),
                ..response::Query::default()
            };
        }
    };

    let height = tendermint::block::Height::try_from(height).expect("height must fit into an i64");
    response::Query {
        code: tendermint::abci::Code::Ok,
        key: request.path.into_bytes().into(),
        value: serde_json::to_vec(&fee_components)
            .expect("object does not contain keys that don't map to json keys")
            .into(),
        height,
        ..response::Query::default()
    }
}

pub(crate) async fn transaction_fee_request(
    storage: Storage,
    request: request::Query,
    _params: Vec<(String, String)>,
) -> response::Query {
    use astria_core::protocol::fees::v1::TransactionFeeResponse;

    let tx = match preprocess_fees_request(&request) {
        Ok(tx) => tx,
        Err(err_rsp) => return err_rsp,
    };

    // use latest snapshot, as this is a query for a transaction fee
    let snapshot = storage.latest_snapshot();
    let height = match snapshot.get_block_height().await {
        Ok(height) => height,
        Err(err) => {
            return response::Query {
                code: Code::Err(AbciErrorCode::INTERNAL_ERROR.value()),
                info: AbciErrorCode::INTERNAL_ERROR.info(),
                log: format!("failed getting block height: {err:#}"),
                ..response::Query::default()
            };
        }
    };

    let fees_with_ibc_denoms =
        match total_fees(tx.actions().iter().map(ActionRef::from), &snapshot).await {
            Ok(fees) => fees,
            Err(err) => {
                return response::Query {
                    code: Code::Err(AbciErrorCode::INTERNAL_ERROR.value()),
                    info: AbciErrorCode::INTERNAL_ERROR.info(),
                    log: format!("failed calculating fees for provided transaction: {err:#}"),
                    ..response::Query::default()
                };
            }
        };

    let mut fees = Vec::with_capacity(fees_with_ibc_denoms.len());
    for (ibc_denom, value) in fees_with_ibc_denoms {
        let trace_denom = match snapshot.map_ibc_to_trace_prefixed_asset(&ibc_denom).await {
            Ok(Some(trace_denom)) => trace_denom,
            Ok(None) => {
                return response::Query {
                    code: Code::Err(AbciErrorCode::INTERNAL_ERROR.value()),
                    info: AbciErrorCode::INTERNAL_ERROR.info(),
                    log: format!(
                        "failed mapping ibc denom to trace denom: {ibc_denom}; asset does not \
                         exist in state"
                    ),
                    ..response::Query::default()
                };
            }
            Err(err) => {
                return response::Query {
                    code: Code::Err(AbciErrorCode::INTERNAL_ERROR.value()),
                    info: AbciErrorCode::INTERNAL_ERROR.info(),
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

fn preprocess_fees_request(request: &request::Query) -> Result<TransactionBody, response::Query> {
    let tx = match RawBody::decode(&*request.data) {
        Ok(tx) => tx,
        Err(err) => {
            return Err(response::Query {
                code: Code::Err(AbciErrorCode::BAD_REQUEST.value()),
                info: AbciErrorCode::BAD_REQUEST.info(),
                log: format!(
                    "failed to decode request data to a protobuf {}: {err:#}",
                    RawBody::full_name()
                ),
                ..response::Query::default()
            });
        }
    };

    let tx = match TransactionBody::try_from_raw(tx) {
        Ok(tx) => tx,
        Err(err) => {
            return Err(response::Query {
                code: Code::Err(AbciErrorCode::BAD_REQUEST.value()),
                info: AbciErrorCode::BAD_REQUEST.info(),
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

#[derive(serde::Serialize)]
struct AllFeeComponents {
    transfer: FetchResult,
    rollup_data_submission: FetchResult,
    ics20_withdrawal: FetchResult,
    init_bridge_account: FetchResult,
    bridge_lock: FetchResult,
    bridge_unlock: FetchResult,
    bridge_transfer: FetchResult,
    bridge_sudo_change: FetchResult,
    ibc_relay: FetchResult,
    validator_update: FetchResult,
    fee_asset_change: FetchResult,
    fee_change: FetchResult,
    ibc_relayer_change: FetchResult,
    sudo_address_change: FetchResult,
    ibc_sudo_change: FetchResult,
    recover_ibc_client: FetchResult,
}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum FetchResult {
    Err(String),
    Missing(&'static str),
    Component(FeeComponent),
}

impl<T> From<eyre::Result<Option<FeeComponents<T>>>> for FetchResult {
    fn from(value: eyre::Result<Option<FeeComponents<T>>>) -> Self {
        match value {
            Ok(Some(val)) => Self::Component(FeeComponent {
                base: val.base(),
                multiplier: val.multiplier(),
            }),
            Ok(None) => Self::Missing("not set"),
            Err(err) => Self::Err(err.to_string()),
        }
    }
}

async fn get_all_fee_components<S: StateRead>(state: &S) -> AllFeeComponents {
    let (
        transfer,
        rollup_data_submission,
        ics20_withdrawal,
        init_bridge_account,
        bridge_lock,
        bridge_unlock,
        bridge_transfer,
        bridge_sudo_change,
        validator_update,
        sudo_address_change,
        ibc_sudo_change,
        ibc_relay,
        ibc_relayer_change,
        fee_asset_change,
        fee_change,
        recover_ibc_client,
    ) = join!(
        state.get_fees::<Transfer>().map(FetchResult::from),
        state
            .get_fees::<RollupDataSubmission>()
            .map(FetchResult::from),
        state.get_fees::<Ics20Withdrawal>().map(FetchResult::from),
        state.get_fees::<InitBridgeAccount>().map(FetchResult::from),
        state.get_fees::<BridgeLock>().map(FetchResult::from),
        state.get_fees::<BridgeUnlock>().map(FetchResult::from),
        state.get_fees::<BridgeTransfer>().map(FetchResult::from),
        state.get_fees::<BridgeSudoChange>().map(FetchResult::from),
        state.get_fees::<ValidatorUpdate>().map(FetchResult::from),
        state.get_fees::<SudoAddressChange>().map(FetchResult::from),
        state.get_fees::<IbcSudoChange>().map(FetchResult::from),
        state.get_fees::<IbcRelay>().map(FetchResult::from),
        state.get_fees::<IbcRelayerChange>().map(FetchResult::from),
        state.get_fees::<FeeAssetChange>().map(FetchResult::from),
        state.get_fees::<FeeChange>().map(FetchResult::from),
        state.get_fees::<RecoverIbcClient>().map(FetchResult::from),
    );
    AllFeeComponents {
        transfer,
        rollup_data_submission,
        ics20_withdrawal,
        init_bridge_account,
        bridge_lock,
        bridge_unlock,
        bridge_transfer,
        bridge_sudo_change,
        ibc_relay,
        validator_update,
        fee_asset_change,
        fee_change,
        ibc_relayer_change,
        sudo_address_change,
        ibc_sudo_change,
        recover_ibc_client,
    }
}

#[derive(serde::Serialize)]
struct FeeComponent {
    base: u128,
    multiplier: u128,
}
