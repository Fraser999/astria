//! After being created the state must be primed with [`State::init`] before any of
//! the other methods can be used. Otherwise, they will panic.
//!
//! The inner state must not be unset after having been set.
use astria_core::{
    execution::v1::{
        Block,
        CommitmentState,
        GenesisInfo,
    },
    primitive::v1::RollupId,
};
use bytes::Bytes;
use sequencer_client::tendermint::block::Height as SequencerHeight;
use tokio::sync::watch::{
    self,
    error::RecvError,
};
use tracing::instrument;

pub(super) fn channel(state: State) -> (StateSender, StateReceiver) {
    let (tx, rx) = watch::channel(state);
    let sender = StateSender {
        inner: tx,
    };
    let receiver = StateReceiver {
        inner: rx,
    };
    (sender, receiver)
}

#[derive(Debug, thiserror::Error)]
#[error(
    "adding sequencer genesis height `{sequencer_genesis_height}` and `{commitment_type}` rollup \
     number `{rollup_number}` overflowed unsigned u32::MAX, the maximum permissible cometbft \
     height"
)]
pub(super) struct InvalidState {
    commitment_type: &'static str,
    sequencer_genesis_height: u64,
    rollup_number: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StateReceiver {
    inner: watch::Receiver<State>,
}

impl StateReceiver {
    pub(crate) fn next_expected_firm_sequencer_height(&self) -> SequencerHeight {
        self.inner
            .borrow()
            .next_expected_firm_sequencer_height()
            .expect(
                "the tracked state must never be set to a genesis/commitment state that cannot be \
                 mapped to a cometbft Sequencer height",
            )
    }

    pub(crate) fn next_expected_soft_sequencer_height(&self) -> SequencerHeight {
        self.inner
            .borrow()
            .next_expected_soft_sequencer_height()
            .expect(
                "the tracked state must never be set to a genesis/commitment state that cannot be \
                 mapped to a cometbft Sequencer height",
            )
    }

    #[instrument(skip_all)]
    pub(crate) async fn next_expected_soft_height_if_changed(
        &mut self,
    ) -> Result<SequencerHeight, RecvError> {
        self.inner.changed().await?;
        Ok(self.next_expected_soft_sequencer_height())
    }
}

pub(super) struct StateSender {
    inner: watch::Sender<State>,
}

fn can_map_firm_to_sequencer_height(
    genesis_info: &GenesisInfo,
    commitment_state: &CommitmentState,
) -> Result<(), InvalidState> {
    let sequencer_genesis_height = genesis_info.sequencer_genesis_block_height();
    let rollup_number = commitment_state.firm().number();
    if map_rollup_number_to_sequencer_height(sequencer_genesis_height, rollup_number).is_none() {
        Err(InvalidState {
            commitment_type: "firm",
            sequencer_genesis_height: sequencer_genesis_height.value(),
            rollup_number: rollup_number.into(),
        })
    } else {
        Ok(())
    }
}

fn can_map_soft_to_sequencer_height(
    genesis_info: &GenesisInfo,
    commitment_state: &CommitmentState,
) -> Result<(), InvalidState> {
    let sequencer_genesis_height = genesis_info.sequencer_genesis_block_height();
    let rollup_number = commitment_state.soft().number();
    if map_rollup_number_to_sequencer_height(sequencer_genesis_height, rollup_number).is_none() {
        Err(InvalidState {
            commitment_type: "soft",
            sequencer_genesis_height: sequencer_genesis_height.value(),
            rollup_number: rollup_number.into(),
        })
    } else {
        Ok(())
    }
}

impl StateSender {
    pub(super) fn subscribe(&self) -> StateReceiver {
        StateReceiver {
            inner: self.inner.subscribe(),
        }
    }

    /// Calculates the maximum allowed spread between firm and soft commitments heights.
    ///
    /// The maximum allowed spread is taken as `max_spread = variance * 6`, where `variance`
    /// is the `celestia_block_variance` as defined in the rollup node's genesis that this
    /// executor/conductor talks to.
    ///
    /// The heuristic 6 is the largest number of Sequencer heights that will be found at
    /// one Celestia height.
    ///
    /// # Panics
    /// Panics if the `u32` underlying the celestia block variance tracked in the state could
    /// not be converted to a `usize`. This should never happen on any reasonable architecture
    /// that Conductor will run on.
    pub(super) fn calculate_max_spread(&self) -> usize {
        usize::try_from(self.celestia_block_variance())
            .expect("converting a u32 to usize should work on any architecture conductor runs on")
            .saturating_mul(6)
    }

    pub(super) fn try_update_commitment_state(
        &mut self,
        commitment_state: CommitmentState,
    ) -> Result<(), InvalidState> {
        let genesis_info = self.genesis_info();
        can_map_firm_to_sequencer_height(&genesis_info, &commitment_state)?;
        can_map_soft_to_sequencer_height(&genesis_info, &commitment_state)?;
        self.inner.send_modify(move |state| {
            state.set_commitment_state(commitment_state);
        });
        Ok(())
    }

    pub(super) fn get(&self) -> tokio::sync::watch::Ref<'_, State> {
        self.inner.borrow()
    }

    pub(super) fn next_expected_firm_sequencer_height(&self) -> SequencerHeight {
        self.inner
            .borrow()
            .next_expected_firm_sequencer_height()
            .expect(
                "the tracked state must never be set to a genesis/commitment state that cannot be \
                 mapped to a cometbft Sequencer height",
            )
    }

    pub(super) fn next_expected_soft_sequencer_height(&self) -> SequencerHeight {
        self.inner
            .borrow()
            .next_expected_soft_sequencer_height()
            .expect(
                "the tracked state must never be set to a genesis/commitment state that cannot be \
                 mapped to a cometbft Sequencer height",
            )
    }
}

macro_rules! forward_impls {
    ($target:ident: $([$fn:ident -> $ret:ty]),*$(,)?) => {
        impl $target {
            $(
            pub(crate) fn $fn(&self) -> $ret {
                self.inner
                    .borrow()
                    .$fn()
                    .clone()
            }
            )*
        }
    }
}

forward_impls!(
    StateSender:
    [genesis_info -> GenesisInfo],
    [firm -> Block],
    [soft -> Block],
    [firm_number -> u32],
    [soft_number -> u32],
    [firm_hash -> Bytes],
    [soft_hash -> Bytes],
    [celestia_block_variance -> u64],
    [rollup_id -> RollupId],
    [sequencer_genesis_block_height -> SequencerHeight],
    [celestia_base_block_height -> u64],
);

forward_impls!(
    StateReceiver:
    [celestia_base_block_height -> u64],
    [celestia_block_variance -> u64],
    [rollup_id -> RollupId],
);

/// `State` tracks the genesis info and commitment state of the remote rollup node.
#[derive(Debug, serde::Serialize)]
pub(super) struct State {
    commitment_state: CommitmentState,
    genesis_info: GenesisInfo,
}

impl State {
    pub(super) fn try_from_genesis_info_and_commitment_state(
        genesis_info: GenesisInfo,
        commitment_state: CommitmentState,
    ) -> Result<Self, InvalidState> {
        can_map_firm_to_sequencer_height(&genesis_info, &commitment_state)?;
        can_map_soft_to_sequencer_height(&genesis_info, &commitment_state)?;
        Ok(State {
            commitment_state,
            genesis_info,
        })
    }

    /// Sets the inner commitment state.
    fn set_commitment_state(&mut self, commitment_state: CommitmentState) {
        self.commitment_state = commitment_state;
    }

    fn genesis_info(&self) -> GenesisInfo {
        self.genesis_info
    }

    fn firm(&self) -> &Block {
        self.commitment_state.firm()
    }

    fn soft(&self) -> &Block {
        self.commitment_state.soft()
    }

    fn firm_number(&self) -> u32 {
        self.commitment_state.firm().number()
    }

    fn soft_number(&self) -> u32 {
        self.commitment_state.soft().number()
    }

    fn firm_hash(&self) -> Bytes {
        self.firm().hash().clone()
    }

    fn soft_hash(&self) -> Bytes {
        self.soft().hash().clone()
    }

    fn celestia_base_block_height(&self) -> u64 {
        self.commitment_state.base_celestia_height()
    }

    fn celestia_block_variance(&self) -> u64 {
        self.genesis_info.celestia_block_variance()
    }

    fn sequencer_genesis_block_height(&self) -> SequencerHeight {
        self.genesis_info.sequencer_genesis_block_height()
    }

    fn rollup_id(&self) -> RollupId {
        self.genesis_info.rollup_id()
    }

    fn next_expected_firm_sequencer_height(&self) -> Option<SequencerHeight> {
        map_rollup_number_to_sequencer_height(
            self.sequencer_genesis_block_height(),
            self.firm_number().saturating_add(1),
        )
    }

    fn next_expected_soft_sequencer_height(&self) -> Option<SequencerHeight> {
        map_rollup_number_to_sequencer_height(
            self.sequencer_genesis_block_height(),
            self.soft_number().saturating_add(1),
        )
    }
}

/// Maps a rollup height to a sequencer height.
///
/// Returns `None` if `sequencer_genesis_height + rollup_number` overflows
/// `u32::MAX`.
fn map_rollup_number_to_sequencer_height(
    sequencer_genesis_height: SequencerHeight,
    rollup_number: u32,
) -> Option<SequencerHeight> {
    let sequencer_genesis_height = sequencer_genesis_height.value();
    let rollup_number: u64 = rollup_number.into();
    let sequencer_height = sequencer_genesis_height.checked_add(rollup_number)?;
    sequencer_height.try_into().ok()
}

/// Maps a sequencer height to a rollup height.
///
/// Returns `None` if `sequencer_height - sequencer_genesis_height` underflows or if
/// the result does not fit in `u32`.
pub(super) fn map_sequencer_height_to_rollup_height(
    sequencer_genesis_height: SequencerHeight,
    sequencer_height: SequencerHeight,
) -> Option<u32> {
    sequencer_height
        .value()
        .checked_sub(sequencer_genesis_height.value())?
        .try_into()
        .ok()
}

#[cfg(test)]
mod tests {
    use astria_core::{
        generated::astria::execution::v1 as raw,
        Protobuf as _,
    };
    use pbjson_types::Timestamp;

    use super::*;

    fn make_commitment_state() -> CommitmentState {
        let firm = Block::try_from_raw(raw::Block {
            number: 1,
            hash: vec![42u8; 32].into(),
            parent_block_hash: vec![41u8; 32].into(),
            timestamp: Some(Timestamp {
                seconds: 123_456,
                nanos: 789,
            }),
            sequencer_block_hash: Bytes::new(),
        })
        .unwrap();
        let soft = Block::try_from_raw(raw::Block {
            number: 2,
            hash: vec![43u8; 32].into(),
            parent_block_hash: vec![42u8; 32].into(),
            timestamp: Some(Timestamp {
                seconds: 123_456,
                nanos: 789,
            }),
            sequencer_block_hash: Bytes::new(),
        })
        .unwrap();
        CommitmentState::builder()
            .firm(firm)
            .soft(soft)
            .base_celestia_height(1u64)
            .build()
            .unwrap()
    }

    fn make_genesis_info() -> GenesisInfo {
        let rollup_id = RollupId::new([24; 32]);
        GenesisInfo::try_from_raw(raw::GenesisInfo {
            rollup_id: Some(rollup_id.to_raw()),
            sequencer_genesis_block_height: 10,
            celestia_block_variance: 0,
        })
        .unwrap()
    }

    fn make_state() -> State {
        State::try_from_genesis_info_and_commitment_state(
            make_genesis_info(),
            make_commitment_state(),
        )
        .unwrap()
    }

    fn make_channel() -> (StateSender, StateReceiver) {
        super::channel(make_state())
    }

    #[test]
    fn next_firm_sequencer_height_is_correct() {
        let (_, rx) = make_channel();
        assert_eq!(
            SequencerHeight::from(12u32),
            rx.next_expected_firm_sequencer_height(),
        );
    }

    #[test]
    fn next_soft_sequencer_height_is_correct() {
        let (_, rx) = make_channel();
        assert_eq!(
            SequencerHeight::from(13u32),
            rx.next_expected_soft_sequencer_height(),
        );
    }

    #[track_caller]
    fn assert_height_is_correct(left: u32, right: u32, expected: u32) {
        assert_eq!(
            SequencerHeight::from(expected),
            map_rollup_number_to_sequencer_height(SequencerHeight::from(left), right)
                .expect("left + right is so small, they should never overflow"),
        );
    }

    #[test]
    fn mapping_rollup_height_to_sequencer_height_works() {
        assert_height_is_correct(0, 0, 0);
        assert_height_is_correct(0, 1, 1);
        assert_height_is_correct(1, 0, 1);
    }
}
