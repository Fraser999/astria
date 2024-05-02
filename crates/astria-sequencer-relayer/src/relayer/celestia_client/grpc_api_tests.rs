use std::{
    io::Write,
    str::FromStr,
};

use astria_core::generated::cosmos::{
    auth::v1beta1::QueryAccountsRequest,
    base::{
        abci::v1beta1::TxResponse,
        query::v1beta1::PageRequest,
        tendermint::v1beta1::{
            service_client::ServiceClient as NodeInfoClient,
            GetNodeInfoRequest,
        },
    },
};
use tendermint::account::Id as AccountId;
use tonic::transport::Endpoint;

use super::*;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn should_fetch_stuff() {
    // mainnet: http://celestia.cumulo.org.es:9090 ("celestia")
    // mocha testnet:  http://grpc-mocha.pops.one:9090 ("mocha-4")
    // arabica devnet: http://validator-1.celestia-arabica-11.com:9090 ("arabica-11")
    let url = std::env::var("CELESTIA_URL").unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());
    let grpc_channel = Endpoint::from_str(&url).unwrap().connect_lazy();

    let mut node_info_client = NodeInfoClient::new(grpc_channel.clone());
    let chain_id: String = node_info_client
        .get_node_info(GetNodeInfoRequest {})
        .await
        .expect("should respond")
        .into_inner()
        .default_node_info
        .expect("`default_node_info` should be `Some`")
        .network;
    println!("chain_id: {chain_id}");

    let mut blob_query_client = BlobQueryClient::new(grpc_channel.clone());
    let gas_per_blob_byte: u32 = blob_query_client
        .params(QueryBlobParamsRequest {})
        .await
        .expect("should respond")
        .into_inner()
        .params
        .expect("`params` should be `Some`")
        .gas_per_blob_byte;
    println!("gas_per_blob_byte: {gas_per_blob_byte}");

    let mut auth_query_client = AuthQueryClient::new(grpc_channel.clone());
    let tx_size_cost_per_byte: u64 = auth_query_client
        .params(QueryAuthParamsRequest {})
        .await
        .expect("should respond")
        .into_inner()
        .params
        .expect("`params` should be `Some`")
        .tx_size_cost_per_byte;
    println!("tx_size_cost_per_byte: {tx_size_cost_per_byte}");

    let mut min_gas_price_client = MinGasPriceClient::new(grpc_channel.clone());
    let min_gas_price_response = min_gas_price_client.config(MinGasPriceRequest {}).await;
    let min_gas_price = min_gas_price_from_response(min_gas_price_response).unwrap();
    println!("min_gas_price: {min_gas_price}");

    let accounts_response = auth_query_client
        .accounts(QueryAccountsRequest {
            pagination: Some(PageRequest {
                key: Bytes::new(),
                offset: 0,
                limit: 8,
                count_total: true,
                reverse: true,
            }),
        })
        .await
        .expect("should respond")
        .into_inner();
    let accounts_as_any = accounts_response.accounts;
    let total_accounts = accounts_response.pagination.unwrap().total;
    println!("got {} accounts of {total_accounts}", accounts_as_any.len());
    let base_account_type_url = BaseAccount::type_url();

    let mut base_account = None;
    for account_as_any in accounts_as_any {
        if account_as_any.type_url != base_account_type_url {
            continue;
        }
        let account = BaseAccount::decode(&*account_as_any.value).unwrap();
        base_account = Some(account.clone());
        let name = account.address;
        let number: u64 = account.account_number;
        let sequence: u64 = account.sequence;
        println!("account {name}, number: {number}, sequence: {sequence}");
        // We could break here - we only need to parse one account.
    }

    // =============================================================================================

    let sequencer_namespace = astria_core::celestia::namespace_v0_from_sha256_of_bytes("test");
    let signing_keys =
        CelestiaKeys::from_path("/home/fraser/Rust/astria/target/celestia.key").unwrap();
    let our_address = bech32_encode(&signing_keys.address);
    let blobs = vec![Blob::new(sequencer_namespace, vec![1; 1_000]).unwrap()];
    let msg_pay_for_blobs = new_msg_pay_for_blobs(blobs.as_slice(), our_address.clone()).unwrap();

    let cost_params =
        CelestiaCostParams::new(gas_per_blob_byte, tx_size_cost_per_byte, min_gas_price);
    let gas_limit = estimate_gas(&msg_pay_for_blobs.blob_sizes, cost_params);
    let base_account = base_account.unwrap();
    let mut tx_client = TxClient::new(grpc_channel.clone());
    let calculated_fee = calculate_fee(cost_params, gas_limit, None);
    assert_ne!(1, calculated_fee);

    let mut tx_response = TxResponse::default();
    for fee in [1, calculated_fee] {
        let signed_tx = new_signed_tx(
            &msg_pay_for_blobs,
            &base_account,
            gas_limit,
            fee,
            chain_id.clone(),
            &signing_keys,
        );

        let blob_tx = new_blob_tx(&signed_tx, blobs.iter());

        println!(
            "broadcasting blob transaction to celestia app, fee: {} utia, gas_limit: {}",
            fee, gas_limit.0
        );
        let request = BroadcastTxRequest {
            tx_bytes: Bytes::from(blob_tx.encode_to_vec()),
            mode: i32::from(BroadcastMode::Sync),
        };
        tx_response = tx_client
            .broadcast_tx(request)
            .await
            .expect("should respond")
            .into_inner()
            .tx_response
            .expect("`tx_response` should be `Some`");

        println!("{tx_response:?}");

        if fee == 1 {
            assert_eq!(INSUFFICIENT_FEE_CODE, tx_response.code);
            let extracted_fee = extract_required_fee_from_log(&tx_response.raw_log).unwrap();
            println!(
                "fee too low - extracted required fee of {extracted_fee} utia from `{}`",
                tx_response.raw_log
            );
            // Too fragile probably.
            assert!(extracted_fee == 8799 || extracted_fee == 176);
        }
    }

    let hash = tx_response.txhash;
    println!("broadcasted, tx_hash: {hash}");

    let request = GetTxRequest {
        hash,
    };
    print!("getting tx");
    for i in 0..20 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let response = tx_client.get_tx(request.clone()).await;
        if let Ok(Some(height)) = block_height_from_response(response) {
            println!("\ngot tx, height: {height}");
            break;
        } else if i == 19 {
            let response = tx_client.get_tx(request).await;
            println!("\n{response:?}");
            panic!();
        } else {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
    }
}

fn bech32_encode(address: &AccountId) -> Bech32Address {
    const ACCOUNT_ADDRESS_PREFIX: bech32::Hrp = bech32::Hrp::parse_unchecked("celestia");
    let encoded_address =
        bech32::encode::<bech32::Bech32>(ACCOUNT_ADDRESS_PREFIX, address.as_bytes()).unwrap();
    Bech32Address(encoded_address)
}
