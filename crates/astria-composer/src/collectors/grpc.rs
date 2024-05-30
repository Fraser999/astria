//! `GrpcCollector` implements the `GrpcCollectorService` rpc service.

use std::{
    collections::HashMap,
    sync::Arc,
};

use astria_core::{
    generated::composer::v1alpha1::{
        grpc_collector_service_server::GrpcCollectorService,
        SubmitRollupTransactionRequest,
        SubmitRollupTransactionResponse,
    },
    primitive::v1::{
        asset::default_native_asset_id,
        RollupId,
    },
    protocol::transaction::v1alpha1::action::SequenceAction,
};
use metrics::Counter;
use tokio::sync::mpsc::error::SendTimeoutError;
use tonic::{
    Request,
    Response,
    Status,
};
use tracing::error;

use crate::{
    collectors::EXECUTOR_SEND_TIMEOUT,
    executor,
};

/// Implements the `GrpcCollectorService` which listens for incoming gRPC requests and
/// sends the Rollup transactions to the Executor. The Executor then sends the transactions
/// to the Astria Shared Sequencer.
pub(crate) struct Grpc {
    executor: executor::Handle,
    txs_received_counters: HashMap<RollupId, Counter>,
    txs_dropped_counters: HashMap<RollupId, Counter>,
}

impl Grpc {
    pub(crate) fn new(
        executor: executor::Handle,
        txs_received_counters: HashMap<RollupId, Counter>,
        txs_dropped_counters: HashMap<RollupId, Counter>,
    ) -> Self {
        Self {
            executor,
            txs_received_counters,
            txs_dropped_counters,
        }
    }

    fn increment_txs_received_counter(&self, id: &RollupId) {
        let Some(counter) = self.txs_received_counters.get(id) else {
            error!(rollup_id = %id, "failed to get grpc txs_received_counter");
            return;
        };
        counter.increment(1);
    }

    fn increment_txs_dropped_counter(&self, id: &RollupId) {
        let Some(counter) = self.txs_dropped_counters.get(id) else {
            error!(rollup_id = %id, "failed to get grpc txs_dropped_counter");
            return;
        };
        counter.increment(1);
    }
}

#[async_trait::async_trait]
impl GrpcCollectorService for Grpc {
    async fn submit_rollup_transaction(
        self: Arc<Self>,
        request: Request<SubmitRollupTransactionRequest>,
    ) -> Result<Response<SubmitRollupTransactionResponse>, Status> {
        let submit_rollup_tx_request = request.into_inner();

        let Ok(rollup_id) = RollupId::try_from_slice(&submit_rollup_tx_request.rollup_id) else {
            return Err(Status::invalid_argument("invalid rollup id"));
        };

        let sequence_action = SequenceAction {
            rollup_id,
            data: submit_rollup_tx_request.data,
            fee_asset_id: default_native_asset_id(),
        };

        self.increment_txs_received_counter(&rollup_id);
        match self
            .executor
            .send_timeout(sequence_action, EXECUTOR_SEND_TIMEOUT)
            .await
        {
            Ok(()) => {}
            Err(SendTimeoutError::Timeout(_seq_action)) => {
                self.increment_txs_dropped_counter(&rollup_id);
                return Err(Status::unavailable("timeout while sending txs to composer"));
            }
            Err(SendTimeoutError::Closed(_seq_action)) => {
                self.increment_txs_dropped_counter(&rollup_id);
                return Err(Status::failed_precondition("composer is not available"));
            }
        }

        Ok(Response::new(SubmitRollupTransactionResponse {}))
    }
}
