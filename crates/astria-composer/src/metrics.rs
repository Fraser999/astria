use std::collections::HashMap;

use astria_core::primitive::v1::RollupId;
use metrics::{
    counter,
    describe_counter,
    describe_gauge,
    describe_histogram,
    gauge,
    histogram,
    Counter,
    Gauge,
    Histogram,
    Unit,
};
use tracing::error;

use crate::Config;

const ROLLUP_CHAIN_NAME_LABEL: &str = "rollup_chain_name";
const ROLLUP_ID_LABEL: &str = "rollup_id";
const COLLECTOR_TYPE_LABEL: &str = "collector_type";

pub struct Metrics {
    geth_txs_received: HashMap<String, Counter>,
    geth_txs_dropped: HashMap<String, Counter>,
    grpc_txs_received: HashMap<RollupId, Counter>,
    grpc_txs_dropped: HashMap<RollupId, Counter>,
    executor: ExecutorMetrics,
}

impl Metrics {
    #[must_use]
    pub fn new(cfg: &Config) -> Self {
        let rollups = cfg.parse_rollups().unwrap_or_else(|error| {
            // Failing to parse the rollups will cause the construction of `Composer` to fail and
            // the process to exit, so just log an error here and otherwise ignore.
            error!(%error, "failed to parse the rollups from config");
            HashMap::new()
        });
        let (geth_txs_received, grpc_txs_received) = register_txs_received(rollups.keys());
        let (geth_txs_dropped, grpc_txs_dropped) = register_txs_dropped(rollups.keys());
        let executor = ExecutorMetrics::new(rollups.keys());

        Self {
            geth_txs_received,
            geth_txs_dropped,
            grpc_txs_received,
            grpc_txs_dropped,
            executor,
        }
    }

    pub(crate) fn geth_txs_received_counter(&self, id: &String) -> Option<Counter> {
        self.geth_txs_received.get(id).cloned()
    }

    pub(crate) fn geth_txs_dropped_counter(&self, id: &String) -> Option<Counter> {
        self.geth_txs_dropped.get(id).cloned()
    }

    pub(crate) fn grpc_txs_received_counters(&self) -> HashMap<RollupId, Counter> {
        self.grpc_txs_received.clone()
    }

    pub(crate) fn grpc_txs_dropped_counters(&self) -> HashMap<RollupId, Counter> {
        self.grpc_txs_dropped.clone()
    }

    pub(crate) fn executor_metrics(&self) -> &ExecutorMetrics {
        &self.executor
    }
}

pub(crate) struct ExecutorMetrics {
    txs_dropped_too_large: HashMap<RollupId, Counter>,
    nonce_fetch_count: Counter,
    nonce_fetch_failure_count: Counter,
    nonce_fetch_latency: Histogram,
    current_nonce: Gauge,
    sequencer_submission_latency: Histogram,
    sequencer_submission_failure_count: Counter,
    txs_per_submission: Histogram,
    bytes_per_submission: Histogram,
}

impl ExecutorMetrics {
    pub(crate) fn new<'a>(rollup_chain_names: impl Iterator<Item = &'a String>) -> Self {
        let txs_dropped_too_large = register_txs_dropped_too_large(rollup_chain_names);
        let nonce_fetch_count = register_nonce_fetch_count();
        let nonce_fetch_failure_count = register_nonce_fetch_failure_count();
        let nonce_fetch_latency = register_nonce_fetch_latency();
        let current_nonce = register_current_nonce();
        let sequencer_submission_latency = register_sequencer_submission_latency();
        let sequencer_submission_failure_count = register_sequencer_submission_failure_count();
        let txs_per_submission = register_txs_per_submission();
        let bytes_per_submission = register_bytes_per_submission();

        Self {
            txs_dropped_too_large,
            nonce_fetch_count,
            nonce_fetch_failure_count,
            nonce_fetch_latency,
            current_nonce,
            sequencer_submission_latency,
            sequencer_submission_failure_count,
            txs_per_submission,
            bytes_per_submission,
        }
    }

    pub(crate) fn txs_dropped_too_large(&self) -> &HashMap<RollupId, Counter> {
        &self.txs_dropped_too_large
    }

    pub(crate) fn nonce_fetch_count(&self) -> &Counter {
        &self.nonce_fetch_count
    }

    pub(crate) fn nonce_fetch_failure_count(&self) -> &Counter {
        &self.nonce_fetch_failure_count
    }

    pub(crate) fn nonce_fetch_latency(&self) -> &Histogram {
        &self.nonce_fetch_latency
    }

    pub(crate) fn current_nonce(&self) -> &Gauge {
        &self.current_nonce
    }

    pub(crate) fn sequencer_submission_latency(&self) -> &Histogram {
        &self.sequencer_submission_latency
    }

    pub(crate) fn sequencer_submission_failure_count(&self) -> &Counter {
        &self.sequencer_submission_failure_count
    }

    pub(crate) fn txs_per_submission(&self) -> &Histogram {
        &self.txs_per_submission
    }

    pub(crate) fn bytes_per_submission(&self) -> &Histogram {
        &self.bytes_per_submission
    }
}

fn register_txs_received<'a>(
    rollup_chain_names: impl Iterator<Item = &'a String>,
) -> (HashMap<String, Counter>, HashMap<RollupId, Counter>) {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_transactions_received");

    describe_counter!(
        METRIC_NAME,
        Unit::Count,
        "The number of transactions successfully received from collectors and bundled, labelled \
         by rollup and collector type"
    );

    let mut geth_counters = HashMap::new();
    let mut grpc_counters = HashMap::new();

    for chain_name in rollup_chain_names {
        let rollup_id = RollupId::from_unhashed_bytes(chain_name.as_bytes());

        let geth_counter = counter!(
            METRIC_NAME,
            ROLLUP_CHAIN_NAME_LABEL => chain_name.clone(),
            ROLLUP_ID_LABEL => rollup_id.to_string(),
            COLLECTOR_TYPE_LABEL => "geth",
        );
        geth_counters.insert(chain_name.clone(), geth_counter.clone());

        let grpc_counter = counter!(
            METRIC_NAME,
            ROLLUP_CHAIN_NAME_LABEL => chain_name.clone(),
            ROLLUP_ID_LABEL => rollup_id.to_string(),
            COLLECTOR_TYPE_LABEL => "grpc",
        );
        grpc_counters.insert(rollup_id, grpc_counter);
    }
    (geth_counters, grpc_counters)
}

fn register_txs_dropped<'a>(
    rollup_chain_names: impl Iterator<Item = &'a String>,
) -> (HashMap<String, Counter>, HashMap<RollupId, Counter>) {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_transactions_dropped");

    describe_counter!(
        METRIC_NAME,
        Unit::Count,
        "The number of transactions dropped by the collectors before bundling, labelled by rollup \
         and collector type"
    );

    let mut geth_counters = HashMap::new();
    let mut grpc_counters = HashMap::new();

    for chain_name in rollup_chain_names {
        let rollup_id = RollupId::from_unhashed_bytes(chain_name.as_bytes());

        let geth_counter = counter!(
            METRIC_NAME,
            ROLLUP_CHAIN_NAME_LABEL => chain_name.clone(),
            ROLLUP_ID_LABEL => rollup_id.to_string(),
            COLLECTOR_TYPE_LABEL => "geth",
        );
        geth_counters.insert(chain_name.clone(), geth_counter.clone());

        let grpc_counter = counter!(
            METRIC_NAME,
            ROLLUP_CHAIN_NAME_LABEL => chain_name.clone(),
            ROLLUP_ID_LABEL => rollup_id.to_string(),
            COLLECTOR_TYPE_LABEL => "grpc",
        );
        grpc_counters.insert(rollup_id, grpc_counter);
    }
    (geth_counters, grpc_counters)
}

fn register_txs_dropped_too_large<'a>(
    rollup_chain_names: impl Iterator<Item = &'a String>,
) -> HashMap<RollupId, Counter> {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_transactions_dropped_too_large");

    describe_counter!(
        METRIC_NAME,
        Unit::Count,
        "The number of transactions dropped because they were too large, labelled by rollup"
    );

    let mut counters = HashMap::new();

    for chain_name in rollup_chain_names {
        let rollup_id = RollupId::from_unhashed_bytes(chain_name.as_bytes());

        let counter = counter!(
            METRIC_NAME,
            ROLLUP_CHAIN_NAME_LABEL => chain_name.clone(),
            ROLLUP_ID_LABEL => rollup_id.to_string(),
        );
        counters.insert(rollup_id, counter);
    }
    counters
}

fn register_nonce_fetch_count() -> Counter {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_nonce_fetch_count");

    describe_counter!(
        METRIC_NAME,
        Unit::Count,
        "The number of times we have attempted to fetch the nonce"
    );
    counter!(METRIC_NAME)
}

fn register_nonce_fetch_failure_count() -> Counter {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_nonce_fetch_failure_count");
    describe_counter!(
        METRIC_NAME,
        Unit::Count,
        "The number of times we have failed to fetch the nonce"
    );
    counter!(METRIC_NAME)
}

fn register_nonce_fetch_latency() -> Histogram {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_nonce_fetch_latency");
    describe_histogram!(
        METRIC_NAME,
        Unit::Seconds,
        "The latency of fetching the nonce, in seconds"
    );
    histogram!(METRIC_NAME)
}

fn register_current_nonce() -> Gauge {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_current_nonce");
    describe_gauge!(METRIC_NAME, Unit::Count, "The current nonce");
    gauge!(METRIC_NAME)
}

fn register_sequencer_submission_latency() -> Histogram {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_sequencer_submission_latency");
    describe_histogram!(
        METRIC_NAME,
        Unit::Seconds,
        "The latency of submitting a transaction to the sequencer, in seconds"
    );
    histogram!(METRIC_NAME)
}

fn register_sequencer_submission_failure_count() -> Counter {
    const METRIC_NAME: &str = concat!(
        env!("CARGO_CRATE_NAME"),
        "_sequencer_submission_failure_count"
    );
    describe_counter!(
        METRIC_NAME,
        Unit::Count,
        "The number of failed transaction submissions to the sequencer"
    );
    counter!(METRIC_NAME)
}

fn register_txs_per_submission() -> Histogram {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_transaction_per_submission");
    describe_histogram!(
        METRIC_NAME,
        Unit::Count,
        "The number of rollup transactions successfully sent to the sequencer in a single \
         submission"
    );
    histogram!(METRIC_NAME)
}

fn register_bytes_per_submission() -> Histogram {
    const METRIC_NAME: &str = concat!(env!("CARGO_CRATE_NAME"), "_bytes_per_submission");
    describe_histogram!(
        METRIC_NAME,
        Unit::Bytes,
        "The total bytes successfully sent to the sequencer in a single submission"
    );
    histogram!(METRIC_NAME)
}
