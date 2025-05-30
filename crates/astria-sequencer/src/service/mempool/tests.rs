use std::num::NonZeroU32;

use tendermint::{
    abci::{
        request::CheckTxKind,
        Code,
    },
    v0_38::abci::request::CheckTx,
};
use tendermint::abci::{MempoolRequest, MempoolResponse};
use astria_core::crypto::ADDRESS_LENGTH;
use astria_core::protocol::transaction::v1::action::Transfer;
use crate::{mempool::RemovalReason, test_utils::Fixture, Metrics};
use crate::test_utils::{astria_address, nria, ALICE};
use tower::Service;

#[tokio::test]
async fn stress() {
    crate::ALLOCATOR.set_limit(3 * 1024 * 1024 * 1024).unwrap();

    telemetry::configure()
        .set_no_otel(true)
        .set_force_stdout(true)
        .set_filter_directives("astria=debug,info")
        .try_init::<Metrics>(&())
        .unwrap();
    println!("STARTED");
    let fixture = Fixture::default_initialized().await;

    let mut check_txs = vec![];
    for nonce in 0..277 {
        let mut builder = fixture
            .checked_tx_builder()
            .with_signer(ALICE.clone())
            .with_nonce(nonce);
        let xfer = Transfer {
            to: astria_address(&[50; ADDRESS_LENGTH]),
            fee_asset: nria().into(),
            asset: nria().into(),
            amount: 1,
        };
        for _ in 0..3620 {
            builder = builder.with_action(xfer.clone());
        }
        let req = CheckTx {
            tx: builder.build().await.encoded_bytes().clone(),
            kind: CheckTxKind::New,
        };
        check_txs.push(req);
    }

    println!("SENDING");

    let storage = fixture.storage();
    let mempool = fixture.mempool();
    let metrics = fixture.metrics();
    let mut service = super::Mempool::new(storage, mempool, metrics);
    for req in check_txs {
        // let rsp =
        //     super::handle_check_tx_request(req, storage.latest_snapshot(), mempool.clone(), metrics).await;
        let MempoolResponse::CheckTx(rsp) =
            service.call(MempoolRequest::CheckTx(req)).await.unwrap();
        assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");
    }

    println!("COMPLETED");
    tokio::time::sleep(std::time::Duration::from_secs(100000000000)).await;
}

/*
#[tokio::test]
async fn future_nonces_are_accepted() {
    // The mempool should allow future nonces.
    let fixture = Fixture::default_initialized().await;

    let the_future_nonce = 10;
    let tx = fixture
        .checked_tx_builder()
        .with_nonce(the_future_nonce)
        .build()
        .await;

    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::New,
    };

    let storage = fixture.storage();
    let mempool = fixture.mempool();
    let metrics = fixture.metrics();
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");

    // mempool should contain single transaction still
    assert_eq!(mempool.len().await, 1);
}

#[tokio::test]
async fn rechecks_pass() {
    // The mempool should not fail rechecks of transactions.
    let fixture = Fixture::default_initialized().await;

    let tx = fixture.checked_tx_builder().build().await;

    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::New,
    };

    let storage = fixture.storage();
    let mempool = fixture.mempool();
    let metrics = fixture.metrics();
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");

    // recheck also passes
    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::Recheck,
    };
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");

    // mempool should contain single transaction still
    assert_eq!(mempool.len().await, 1);
}

#[tokio::test]
async fn can_reinsert_after_recheck_fail() {
    // The mempool should be able to re-insert a transaction after a recheck fails due to the
    // transaction being removed from the appside mempool. This is to allow users to re-insert
    // if they wish to do so.
    let fixture = Fixture::default_initialized().await;

    let tx = fixture.checked_tx_builder().build().await;
    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::New,
    };

    let storage = fixture.storage();
    let mempool = fixture.mempool();
    let metrics = fixture.metrics();
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");

    // remove the transaction from the mempool to make recheck fail
    mempool
        .remove_tx_invalid(tx.clone(), RemovalReason::Expired)
        .await;

    // see recheck fails
    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::Recheck,
    };
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Err(NonZeroU32::new(9).unwrap()), "{rsp:#?}");

    // can re-insert the transaction after first recheck fail
    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::New,
    };
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");
}

#[tokio::test]
async fn recheck_adds_non_tracked_tx() {
    // The mempool should be able to insert a transaction on recheck if it isn't in the mempool.
    // This could happen in the case of a sequencer restart as the cometbft mempool persists but
    // the appside one does not.
    let fixture = Fixture::default_initialized().await;

    let tx = fixture.checked_tx_builder().build().await;

    let req = CheckTx {
        tx: tx.encoded_bytes().clone(),
        kind: CheckTxKind::Recheck,
    };

    // recheck should pass and add transaction to mempool
    let storage = fixture.storage();
    let mempool = fixture.mempool();
    let metrics = fixture.metrics();
    let rsp =
        super::handle_check_tx_request(req, storage.latest_snapshot(), &mempool, metrics).await;
    assert_eq!(rsp.code, Code::Ok, "{rsp:#?}");

    // mempool should contain single transaction still
    assert_eq!(mempool.len().await, 1);
}
*/
