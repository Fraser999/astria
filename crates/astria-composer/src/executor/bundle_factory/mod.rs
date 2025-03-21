//! This module is responsible for bundling sequence actions into bundles that can be
//! submitted to the sequencer.
use std::{
    collections::{
        HashMap,
        VecDeque,
    },
    mem,
};

use astria_core::{
    primitive::v1::RollupId,
    protocol::transaction::v1::{
        action::RollupDataSubmission,
        Action,
        TransactionBody,
    },
    Protobuf as _,
};
use serde::ser::{
    Serialize,
    SerializeStruct as _,
};
use tracing::trace;

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
enum SizedBundleError {
    #[error("bundle does not have enough space left for the given sequence action")]
    NotEnoughSpace(RollupDataSubmission),
    #[error("sequence action is larger than the max bundle size")]
    SequenceActionTooLarge(RollupDataSubmission),
}

pub(super) struct SizedBundleReport<'a>(pub(super) &'a SizedBundle);

impl Serialize for SizedBundleReport<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut report = serializer.serialize_struct("SizedBundleReport", 2)?;
        report.serialize_field("size", &self.0.curr_size)?;
        report.serialize_field("rollup_counts", &self.0.rollup_counts)?;
        report.end()
    }
}

/// A bundle sequence actions to be submitted to the sequencer. Maintains the total size of the
/// bytes pushed to it and enforces a max size in bytes passed in the constructor. If an incoming
/// `seq_action` won't fit in the buffer it is flushed and a new bundle is started.
#[derive(Clone)]
pub(super) struct SizedBundle {
    /// The buffer of actions
    buffer: Vec<Action>,
    /// The current size of the bundle in bytes. This is equal to the sum of the size of the
    /// `seq_action`s + `ROLLUP_ID_LEN` for each.
    curr_size: usize,
    /// The max bundle size in bytes to enforce.
    max_size: usize,
    /// Mapping of rollup id to the number of sequence actions for that rollup id in the bundle.
    rollup_counts: HashMap<RollupId, usize>,
}

impl SizedBundle {
    /// Create a new empty bundle with the given max size.
    fn new(max_size: usize) -> Self {
        Self {
            buffer: vec![],
            curr_size: 0,
            max_size,
            rollup_counts: HashMap::new(),
        }
    }

    /// Constructs a [`Body`] from the actions contained in the bundle and provided parameters.
    ///
    /// # Panics
    /// Method is expected to never panic because only `Sequence` actions are added to the bundle,
    /// which should produce a valid variant of the [`action::Group`] type.
    pub(super) fn to_transaction_body(&self, nonce: u32, chain_id: &str) -> TransactionBody {
        TransactionBody::builder()
            .actions(self.buffer.clone())
            .chain_id(chain_id)
            .nonce(nonce)
            .try_build()
            .expect(
                "method is expected to never panic because only `SequenceActions` are added to \
                 the bundle, which should produce a valid variant of the `action::Group` type; \
                 this is checked by `tests::transaction_construction_should_not_panic",
            )
    }

    /// Buffer `seq_action` into the bundle.
    /// # Errors
    /// - `seq_action` is beyond the max size allowed for the entire bundle
    /// - `seq_action` does not fit in the remaining space in the bundle
    fn try_push(&mut self, seq_action: RollupDataSubmission) -> Result<(), SizedBundleError> {
        let seq_action_size = encoded_len(&seq_action);

        if seq_action_size > self.max_size {
            return Err(SizedBundleError::SequenceActionTooLarge(seq_action));
        }

        let new_size = self.curr_size.saturating_add(seq_action_size);

        if new_size > self.max_size {
            return Err(SizedBundleError::NotEnoughSpace(seq_action));
        }

        self.rollup_counts
            .entry(seq_action.rollup_id)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        self.buffer.push(Action::RollupDataSubmission(seq_action));
        self.curr_size = new_size;

        Ok(())
    }

    /// Replace self with a new empty bundle, returning the old bundle.
    fn flush(&mut self) -> SizedBundle {
        mem::replace(self, Self::new(self.max_size))
    }

    /// Returns the current size of the bundle.
    pub(super) fn get_size(&self) -> usize {
        self.curr_size
    }

    /// Returns the number of sequence actions in the bundle.
    pub(super) fn actions_count(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the bundle is empty.
    pub(super) fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum BundleFactoryError {
    #[error("sequence action is larger than the max bundle size. seq_action size: {size}")]
    SequenceActionTooLarge { size: usize, max_size: usize },
    #[error(transparent)]
    FinishedQueueFull(Box<FinishedQueueFull>),
}

#[derive(Debug, thiserror::Error)]
#[error(
    "finished bundle queue is at capacity and the sequence action does not fit in the current \
     bundle. finished queue capacity: {finished_queue_capacity}, curr bundle size: \
     {curr_bundle_size}, sequence action size: {sequence_action_size}"
)]
pub(super) struct FinishedQueueFull {
    curr_bundle_size: usize,
    finished_queue_capacity: usize,
    sequence_action_size: usize,
    seq_action: RollupDataSubmission,
}

impl From<FinishedQueueFull> for BundleFactoryError {
    fn from(value: FinishedQueueFull) -> Self {
        Self::FinishedQueueFull(Box::new(value))
    }
}

/// Manages the bundling of sequence actions into `SizedBundle`s. A `Vec<Action>` is flushed and
/// added to the `finished` queue when an incoming `SequenceAction` won't fit in the current bundle.
/// The `finished` queue operates in FIFO order, where `Vec<Action>`s are added to the back and
/// taken off from the front.
pub(super) struct BundleFactory {
    /// The current bundle being built.
    curr_bundle: SizedBundle,
    /// The queue of bundles that have been built but not yet sent to the sequencer.
    finished: VecDeque<SizedBundle>,
    /// Max amount of `SizedBundle`s that can be in the `finished` queue.
    finished_queue_capacity: usize,
}

impl BundleFactory {
    pub(super) fn new(max_bytes_per_bundle: usize, finished_queue_capacity: usize) -> Self {
        Self {
            curr_bundle: SizedBundle::new(max_bytes_per_bundle),
            finished: VecDeque::new(),
            finished_queue_capacity,
        }
    }

    /// Buffer `seq_action` into the current bundle. If the bundle won't fit `seq_action`, flush
    /// `curr_bundle` into the `finished` queue and start a new bundle, unless the `finished` queue
    /// is at capacity.
    pub(super) fn try_push(
        &mut self,
        seq_action: RollupDataSubmission,
    ) -> Result<(), BundleFactoryError> {
        let seq_action = with_ibc_prefixed(seq_action);
        let seq_action_size = encoded_len(&seq_action);

        match self.curr_bundle.try_push(seq_action) {
            Err(SizedBundleError::SequenceActionTooLarge(_seq_action)) => {
                // reject the sequence action if it is larger than the max bundle size
                Err(BundleFactoryError::SequenceActionTooLarge {
                    size: seq_action_size,
                    max_size: self.curr_bundle.max_size,
                })
            }
            Err(SizedBundleError::NotEnoughSpace(seq_action)) => {
                if self.finished.len() >= self.finished_queue_capacity {
                    Err(FinishedQueueFull {
                        curr_bundle_size: self.curr_bundle.curr_size,
                        finished_queue_capacity: self.finished_queue_capacity,
                        sequence_action_size: seq_action_size,
                        seq_action,
                    }
                    .into())
                } else {
                    // if the bundle is full, flush it and start a new one
                    self.finished.push_back(self.curr_bundle.flush());
                    self.curr_bundle.try_push(seq_action).expect(
                        "seq_action should not be larger than max bundle size, this is a bug",
                    );
                    trace!(
                        new_bundle_size = self.curr_bundle.curr_size,
                        seq_action_size = seq_action_size,
                        finished_queue.current_size = self.finished.len(),
                        finished_queue.capacity = self.finished_queue_capacity,
                        "created new bundle and bundled new sequence action"
                    );
                    Ok(())
                }
            }
            Ok(()) => {
                trace!(
                    new_bundle_size = self.curr_bundle.curr_size,
                    seq_action_size = seq_action_size,
                    "bundled new sequence action"
                );
                Ok(())
            }
        }
    }

    /// Returns a handle to the next finished bundle if it exists.
    ///
    /// The bundle is only removed from the factory on calling [`NextFinishedBundle::pop`].
    /// This method primarily exists to work around async cancellation.
    pub(super) fn next_finished(&mut self) -> Option<NextFinishedBundle> {
        if self.finished.is_empty() {
            None
        } else {
            Some(NextFinishedBundle {
                bundle_factory: self,
            })
        }
    }

    /// Immediately the currently aggregating bundle.
    ///
    /// Returns an empty bundle if there are no bundled transactions.
    pub(super) fn pop_now(&mut self) -> SizedBundle {
        self.finished
            .pop_front()
            .or_else(|| Some(self.curr_bundle.flush()))
            .unwrap_or(SizedBundle::new(self.curr_bundle.max_size))
    }

    pub(super) fn is_full(&self) -> bool {
        self.finished.len() >= self.finished_queue_capacity
    }
}

pub(super) struct NextFinishedBundle<'a> {
    bundle_factory: &'a mut BundleFactory,
}

impl NextFinishedBundle<'_> {
    pub(super) fn pop(self) -> SizedBundle {
        self.bundle_factory
            .finished
            .pop_front()
            .expect("next bundle exists. this is a bug.")
    }
}

fn with_ibc_prefixed(action: RollupDataSubmission) -> RollupDataSubmission {
    RollupDataSubmission {
        fee_asset: action.fee_asset.to_ibc_prefixed().into(),
        ..action
    }
}

fn encoded_len(action: &RollupDataSubmission) -> usize {
    use prost::Message as _;
    action.to_raw().encoded_len()
}
