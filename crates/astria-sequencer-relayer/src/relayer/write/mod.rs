//! A task writing sequencer blocks to Celestia.
//!
//! [`BlobSubmitter`] receives [`SequencerBlock`]s over a channel,
//! converts them to Celestia [`Blob`]s, and writes them to Celestia
//! using the `blob.Submit` API.
//!
//! [`BlobSubmitter`] submits converted blobs strictly in the order it
//! receives blocks and imposes no extra ordering. This means that if
//! another task sends sequencer blocks ordered by their heights, then
//! they will be written in that order.
use std::{
    sync::Arc,
    time::{
        Duration,
        SystemTime,
    },
};

use astria_eyre::eyre::{
    self,
    bail,
    WrapErr as _,
};
use celestia_types::Blob;
use futures::{
    future::{
        Fuse,
        FusedFuture as _,
    },
    FutureExt as _,
};
use sequencer_client::SequencerBlock;
use tendermint::block::Height as SequencerHeight;
use thiserror::Error;
use tokio::{
    select,
    sync::{
        mpsc::{
            self,
            error::{
                SendError,
                TrySendError,
            },
        },
        watch,
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{
    debug,
    error,
    info,
    instrument,
    warn,
    Instrument,
    Span,
};

use super::{
    celestia_client::CelestiaClient,
    BlobTxHash,
    BuilderError,
    CelestiaClientBuilder,
    SubmissionState,
    TrySubmitError,
};
use crate::{
    metrics::Metrics,
    IncludeRollup,
};

mod conversion;
use conversion::NextSubmission;

#[derive(Clone)]
pub(super) struct BlobSubmitterHandle {
    tx: mpsc::Sender<SequencerBlock>,
}

impl BlobSubmitterHandle {
    /// Send a block to the blob submitter immediately.
    ///
    /// This is a thin wrapper around [`mpsc::Sender::try_send`].
    // allow: just forwarding the error type
    #[allow(clippy::result_large_err)]
    pub(super) fn try_send(
        &self,
        block: SequencerBlock,
    ) -> Result<(), TrySendError<SequencerBlock>> {
        self.tx.try_send(block)
    }

    /// Sends a block to the blob submitter.
    ///
    /// This is a thin wrapper around [`mpsc::Sender::send`].
    // allow: just forwarding the error type
    #[allow(clippy::result_large_err)]
    pub(super) async fn send(
        &self,
        block: SequencerBlock,
    ) -> Result<(), SendError<SequencerBlock>> {
        self.tx.send(block).await
    }
}

pub(super) struct BlobSubmitter {
    /// The builder for a client to submit blobs to Celestia.
    client_builder: CelestiaClientBuilder,

    /// The channel over which sequencer blocks are received.
    blocks: mpsc::Receiver<SequencerBlock>,

    /// The accumulator of all data that will be submitted to Celestia on the next submission.
    next_submission: NextSubmission,

    /// The state of the relayer.
    state: Arc<super::State>,

    /// Tracks the submission state and writes it to disk before and after each Celestia
    /// submission.
    submission_state: SubmissionState,

    /// The shutdown token to signal that blob submitter should finish its current submission and
    /// exit.
    submitter_shutdown_token: CancellationToken,

    /// A block that could not be added to `next_submission` because it would overflow its
    /// hardcoded limit.
    pending_block: Option<SequencerBlock>,

    metrics: &'static Metrics,
}

impl BlobSubmitter {
    pub(super) fn new(
        client_builder: CelestiaClientBuilder,
        rollup_filter: IncludeRollup,
        state: Arc<super::State>,
        submission_state: SubmissionState,
        submitter_shutdown_token: CancellationToken,
        metrics: &'static Metrics,
    ) -> (Self, BlobSubmitterHandle) {
        // XXX: The channel size here is just a number. It should probably be based on some
        // heuristic about the number of expected blobs in a block.
        let (tx, rx) = mpsc::channel(128);
        let submitter = Self {
            client_builder,
            blocks: rx,
            next_submission: NextSubmission::new(rollup_filter, metrics),
            state,
            submission_state,
            submitter_shutdown_token,
            pending_block: None,
            metrics,
        };
        let handle = BlobSubmitterHandle {
            tx,
        };
        (submitter, handle)
    }

    pub(super) async fn run(mut self) -> eyre::Result<()> {
        let init_result = select!(
            () = self.submitter_shutdown_token.cancelled() => return Ok(()),
            init_result = init_with_retry(self.client_builder.clone()) => init_result,
        );
        let client = init_result.map_err(|error| {
            let message = "failed to initialize celestia client";
            error!(%error, message);
            error.wrap_err(message)
        })?;

        if let Some((blob_tx_hash, submitted_at)) = self.submission_state.tx_to_confirm() {
            self.submission_state = try_confirm_submission_from_last_session(
                client.clone(),
                blob_tx_hash,
                submitted_at,
                self.state.clone(),
                self.submission_state.clone(),
                self.metrics,
            )
            .await?;
        }

        // A submission to Celestia that is currently in-flight.
        let mut ongoing_submission = Fuse::terminated();

        let reason = loop {
            select!(
                biased;

                () = self.submitter_shutdown_token.cancelled() => {
                    info!("shutdown signal received");
                    break Ok("received shutdown signal");
                }

                // handle result of submitting blocks to Celestia, if in flight
                submission_result = &mut ongoing_submission,
                                    if !ongoing_submission.is_terminated()
                                    =>
                {
                    // XXX: Breaks the select-loop and returns. With the current retry-logic in
                    // `submit_blobs` this happens after u32::MAX retries which is effectively never.
                    // self.submission_state = match submission_result.wrap_err("failed submitting blocks to Celestia")
                    self.submission_state = match submission_result {
                        Ok(state) => state,
                        Err(err) => {
                            // Use `wrap_err` on the return break value. Using it on the match-value causes
                            // type inference to fail.
                            break Err(err).wrap_err("failed submitting blocks to Celestia");
                        }
                    };
                }

                // submit blocks to Celestia, if no submission in flight
                Some(submission) = self.next_submission.take(),
                                    if ongoing_submission.is_terminated()
                                    => {
                    ongoing_submission = submit_blobs(
                        client.clone(),
                        submission,
                        self.state.clone(),
                        self.submission_state.clone(),
                        self.metrics,
                    ).boxed().fuse();
                    if let Some(block) = self.pending_block.take() {
                        if let Err(error) = self.add_sequencer_block_to_next_submission(block) {
                            break Err(error).wrap_err(
                                "critically failed adding Sequencer block to next submission"
                            );
                        }
                    }
                }

                // add new blocks to the next submission if there is space.
                Some(block) = self.blocks.recv(), if self.has_capacity() => {
                    if let Err(error) = self.add_sequencer_block_to_next_submission(block) {
                        break Err(error).wrap_err(
                            "critically failed adding Sequencer block to next submission"
                        );
                    }
                }

            );
        };

        match &reason {
            Ok(reason) => info!(reason, "starting shutdown"),
            Err(reason) => error!(%reason, "starting shutdown"),
        }

        if ongoing_submission.is_terminated() {
            info!("no submissions to Celestia were in flight, exiting now");
        } else {
            info!("a submission to Celestia is in flight; waiting for it to finish");
            if let Err(error) = ongoing_submission.await {
                error!(%error, "last submission to Celestia failed before exiting");
            }
        }
        reason.map(|_| ())
    }

    #[instrument(skip_all, fields(sequencer_height = block.height().value()), err)]
    fn add_sequencer_block_to_next_submission(
        &mut self,
        block: SequencerBlock,
    ) -> eyre::Result<()> {
        match self.next_submission.try_add(block) {
            Ok(()) => debug!("block was scheduled for next submission"),
            Err(conversion::TryAddError::Full(block)) => {
                debug!(
                    "block was rejected from next submission because it would overflow the \
                     maximum payload size; pushing back until the next submission is done"
                );
                self.pending_block = Some(*block);
            }
            Err(err) => {
                return Err(err).wrap_err("failed adding sequencer block to next submission");
            }
        }
        Ok(())
    }

    /// Returns if the next submission still has capacity.
    fn has_capacity(&self) -> bool {
        // The next submission has capacity if no block was rejected.
        self.pending_block.is_none()
    }
}

/// Tries to confirm the last attempted submission of the previous session.
///
/// This should only be called where submission state on startup is `Prepared`, meaning we don't yet
/// know whether that final submission attempt succeeded or not.
///
/// Internally, this polls `GetTx` for up to one minute.  The returned `SubmissionState` is
/// guaranteed to be in `Finished` state, either holding the heights of the previously prepared
/// submission if confirmed by Celestia, or holding the heights of the last known confirmed
/// submission in the case of timing out.
#[instrument(skip_all)]
async fn try_confirm_submission_from_last_session(
    mut client: CelestiaClient,
    blob_tx_hash: BlobTxHash,
    submitted_at: SystemTime,
    state: Arc<super::State>,
    submission_state: SubmissionState,
    metrics: &'static Metrics,
) -> eyre::Result<SubmissionState> {
    info!(tx_hash = %blob_tx_hash, "confirming submission of last BlobTx from previous session");

    // We want to poll `GetTx` for at least 15 seconds, but up to a maximum of a minute from when
    // the previous submission was made.
    let timeout = std::cmp::max(
        Duration::from_secs(15),
        Duration::from_secs(60).saturating_sub(submitted_at.elapsed().unwrap_or_default()),
    );

    let final_state =
        if let Some(celestia_height) = client.try_confirm_submission(blob_tx_hash, timeout).await {
            info!(%celestia_height, "confirmed previous session submitted blobs to Celestia");
            submission_state.finish(celestia_height)?
        } else {
            info!(
                "previous session's last submission was not completed; continuing from last \
                 confirmed submission"
            );
            submission_state.revert_from_prepared_to_finished()?
        };

    if let Some(completed_submission) = final_state.last_completed_submission() {
        metrics.absolute_set_sequencer_submission_height(
            completed_submission.sequencer_height().value(),
        );
        metrics.absolute_set_celestia_submission_height(completed_submission.celestia_height());
        state.set_latest_confirmed_celestia_height(completed_submission.celestia_height());
    } else {
        error!(%final_state, "should have a last completed submission available");
    }

    Ok(final_state)
}

/// Submits new blobs Celestia.
#[instrument(skip_all)]
async fn submit_blobs(
    client: CelestiaClient,
    data: conversion::Submission,
    state: Arc<super::State>,
    submission_state: SubmissionState,
    metrics: &'static Metrics,
) -> eyre::Result<SubmissionState> {
    info!(
        blocks = %telemetry::display::json(&data.input_metadata()),
        total_data_uncompressed_size = data.uncompressed_size(),
        total_data_compressed_size = data.compressed_size(),
        compression_ratio = data.compression_ratio(),
        "initiated submission of sequencer blocks converted to Celestia blobs",
    );

    let start = std::time::Instant::now();

    metrics.record_bytes_per_celestia_tx(data.compressed_size());
    metrics.set_compression_ratio_for_astria_block(data.compression_ratio());
    metrics.increment_celestia_submission_count();
    metrics.record_blocks_per_celestia_tx(data.num_blocks());
    metrics.record_blobs_per_celestia_tx(data.num_blobs());

    let largest_sequencer_height = data.greatest_sequencer_height();
    let blobs = data.into_blobs();

    let final_state = submit_with_retry(
        client,
        blobs,
        state.clone(),
        submission_state,
        largest_sequencer_height,
        metrics,
    )
    .await
    .map_err(|error| {
        let message = "failed submitting blobs to Celestia";
        error!(%error, message);
        error.wrap_err(message)
    })?;

    let celestia_height = match final_state.last_completed_submission() {
        Some(completed_submission) => completed_submission.celestia_height(),
        None => bail!(
            "should have a last completed submission available: {}",
            final_state
        ),
    };

    metrics.absolute_set_sequencer_submission_height(largest_sequencer_height.value());
    metrics.absolute_set_celestia_submission_height(celestia_height);
    metrics.record_celestia_submission_latency(start.elapsed());

    info!(%celestia_height, "successfully submitted blobs to Celestia");

    state.set_celestia_connected(true);
    state.set_latest_confirmed_celestia_height(celestia_height);

    Ok(final_state)
}

#[instrument(skip_all)]
async fn init_with_retry(client_builder: CelestiaClientBuilder) -> eyre::Result<CelestiaClient> {
    let span = Span::current();

    let initial_retry_delay = Duration::from_secs(1);
    let retry_config = tryhard::RetryFutureConfig::new(u32::MAX)
        .max_delay(Duration::from_secs(30))
        .custom_backoff(|attempt: u32, error: &BuilderError| {
            if matches!(error, BuilderError::MismatchedCelestiaChainId { .. }) {
                // We got a good response from the Celestia app, but this is an unrecoverable error.
                return tryhard::RetryPolicy::Break;
            }
            // This is equivalent to the `exponential_backoff` policy.  Note that `max_delay`
            // above is still respected regardless of what we return here.
            let delay =
                initial_retry_delay.saturating_mul(2_u32.saturating_pow(attempt.saturating_sub(1)));
            tryhard::RetryPolicy::Delay(delay)
        })
        .on_retry(
            |attempt: u32, next_delay: Option<Duration>, error: &BuilderError| {
                let wait_duration = next_delay
                    .map(humantime::format_duration)
                    .map(tracing::field::display);
                warn!(
                    parent: &span,
                    attempt,
                    wait_duration,
                    error = %eyre::Report::new(error.clone()),
                    "failed to initialize celestia client; retrying after backoff",
                );
                futures::future::ready(())
            },
        );

    let celestia_client = tryhard::retry_fn(move || client_builder.clone().try_build())
        .with_config(retry_config)
        .in_current_span()
        .await
        .wrap_err("failed to initialize celestia client")?;
    info!("initialized celestia client");
    Ok(celestia_client)
}

#[derive(Error, Debug)]
enum SubmissionError {
    #[error(transparent)]
    TrySubmit(#[from] TrySubmitError),
    #[error(transparent)]
    SubmissionState(#[from] eyre::Report),
}

#[instrument(skip_all)]
async fn submit_with_retry(
    client: CelestiaClient,
    blobs: Vec<Blob>,
    state: Arc<super::State>,
    submission_state: SubmissionState,
    largest_sequencer_height: SequencerHeight,
    metrics: &'static Metrics,
) -> eyre::Result<SubmissionState> {
    // Moving the span into `on_retry`, because tryhard spawns these in a tokio
    // task, losing the span.
    let span = Span::current();

    // Create a watch channel to allow the `on_retry` function to provide the received
    // `TrySubmitError` to the next attempt of the `retry_fn`.
    let (last_error_sender, last_error_receiver) = watch::channel(None);

    let initial_retry_delay = Duration::from_millis(100);
    let retry_config = tryhard::RetryFutureConfig::new(u32::MAX)
        // 12 seconds is the Celestia block time.
        .max_delay(Duration::from_secs(12))
        .custom_backoff(|attempt: u32, error: &SubmissionError| {
            if matches!(error, SubmissionError::SubmissionState(_)) {
                // We failed updating the submission state; this is an unrecoverable error.
                return tryhard::RetryPolicy::Break;
            }
            // This is equivalent to the `exponential_backoff` policy.  Note that `max_delay`
            // above is still respected regardless of what we return here.
            let delay =
                initial_retry_delay.saturating_mul(2_u32.saturating_pow(attempt.saturating_sub(1)));
            tryhard::RetryPolicy::Delay(delay)
        })
        .on_retry(
            |attempt: u32, next_delay: Option<Duration>, error: &SubmissionError| {
                metrics.increment_celestia_submission_failure_count();

                let state = Arc::clone(&state);
                state.set_celestia_connected(false);
                let try_submit_error = match error {
                    SubmissionError::TrySubmit(try_submit_error) => try_submit_error,
                    SubmissionError::SubmissionState(_) => {
                        unreachable!("we don't retry in `custom_backoff` for this error variant");
                    }
                };
                let _ = last_error_sender.send(Some(try_submit_error.clone()));

                let wait_duration = next_delay
                    .map(humantime::format_duration)
                    .map(tracing::field::display);

                warn!(
                    parent: &span,
                    attempt,
                    wait_duration,
                    error = %eyre::Report::new(try_submit_error.clone()),
                    "failed submitting blobs to Celestia; retrying after backoff",
                );
                futures::future::ready(())
            },
        );

    let blobs = Arc::new(blobs);

    let final_state = tryhard::retry_fn(move || {
        try_submit(
            client.clone(),
            blobs.clone(),
            submission_state.clone(),
            largest_sequencer_height,
            last_error_receiver.clone(),
        )
    })
    .with_config(retry_config)
    .in_current_span()
    .await
    .wrap_err("finished trying to submit")?;
    Ok(final_state)
}

async fn try_submit(
    mut client: CelestiaClient,
    blobs: Arc<Vec<Blob>>,
    submission_state: SubmissionState,
    largest_sequencer_height: SequencerHeight,
    last_error_receiver: watch::Receiver<Option<TrySubmitError>>,
) -> Result<SubmissionState, SubmissionError> {
    // Get the error from the last attempt to `try_submit`.
    let maybe_last_error = last_error_receiver.borrow().clone();

    // If error is broadcast timeout, reset state to prepared and try to confirm submission.
    if matches!(
        &maybe_last_error,
        Some(TrySubmitError::FailedToBroadcastTx(error)) if error.is_timeout()
    ) {
        if let Some(final_state) =
            try_confirm_submission_from_failed_attempt(client.clone(), submission_state.clone())
                .await?
        {
            return Ok(final_state);
        }
    }

    let started_state = submission_state
        .start(largest_sequencer_height)
        .await
        .map_err(|error| {
            error!(%error, "failed to initialize submission; abandoning");
            SubmissionError::SubmissionState(error)
        })?;

    let (blob_tx_hash, blob_tx) = client.try_prepare(blobs, maybe_last_error).await?;

    let prepared_state = started_state.prepare(blob_tx_hash).await?;

    let celestia_height = client.try_submit(blob_tx_hash, blob_tx).await?;

    let final_state = prepared_state.finish(celestia_height)?;
    Ok(final_state)
}

/// Tries to confirm the submission from a failed previous attempt.  Returns `Some` if the
/// submission is confirmed, or `None` if not.
///
/// This should only be called where submission state is `Prepared`, meaning we don't yet
/// know whether that previous submission attempt succeeded or not.
#[instrument(skip_all)]
async fn try_confirm_submission_from_failed_attempt(
    mut client: CelestiaClient,
    submission_state: SubmissionState,
) -> eyre::Result<Option<SubmissionState>> {
    let prepared_state = submission_state.read_from_disk().await?;
    let blob_tx_hash = if let Some((tx_hash, _submitted_at)) = prepared_state.tx_to_confirm() {
        info!(%tx_hash, "confirming submission of last BlobTx from previous attempt");
        tx_hash
    } else {
        error!(
            state_on_disk = %prepared_state,
            "if broadcast tx timed out, state on disk should be `prepared`"
        );
        return Ok(None);
    };

    if let Some(celestia_height) = client
        .try_confirm_submission(blob_tx_hash, Duration::from_secs(60))
        .await
    {
        info!(%celestia_height, "confirmed previous attempt submitted blobs to Celestia");
        return Ok(Some(prepared_state.finish(celestia_height)?));
    }

    info!("previous attempt's last submission was not completed; starting resubmission");
    Ok(None)
}
