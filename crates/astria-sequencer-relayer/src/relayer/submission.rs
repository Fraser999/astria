//! Tracks the current submission state of sequencer-relayer and syncs it to disk.

use std::{
    fmt::{
        self,
        Display,
        Formatter,
    },
    path::{
        Path,
        PathBuf,
    },
    time::SystemTime,
};

use astria_eyre::eyre::{
    self,
    bail,
    ensure,
    WrapErr as _,
};
use serde::{
    Deserialize,
    Serialize,
};
use tendermint::block::Height as SequencerHeight;
use tracing::debug;

use super::BlobTxHash;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct CompletedSubmission {
    celestia_height: u64,
    #[serde(with = "as_number")]
    sequencer_height: SequencerHeight,
}

impl CompletedSubmission {
    pub(super) fn celestia_height(&self) -> u64 {
        self.celestia_height
    }

    pub(super) fn sequencer_height(&self) -> SequencerHeight {
        self.sequencer_height
    }

    fn new(celestia_height: u64, sequencer_height: SequencerHeight) -> Self {
        Self {
            celestia_height,
            sequencer_height,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
enum State {
    Fresh,
    Started {
        last_submission: CompletedSubmission,
    },
    Prepared {
        #[serde(with = "as_number")]
        sequencer_height: SequencerHeight,
        last_submission: CompletedSubmission,
        blob_tx_hash: BlobTxHash,
        #[serde(with = "humantime_serde")]
        at: SystemTime,
    },
    Finished(CompletedSubmission),
}

impl State {
    fn new_started(last_submission: CompletedSubmission) -> Self {
        Self::Started {
            last_submission,
        }
    }

    fn new_prepared(
        sequencer_height: SequencerHeight,
        last_submission: CompletedSubmission,
        blob_tx_hash: BlobTxHash,
    ) -> Self {
        Self::Prepared {
            sequencer_height,
            last_submission,
            blob_tx_hash,
            at: SystemTime::now(),
        }
    }

    async fn read(source: &Path) -> eyre::Result<Self> {
        let contents = tokio::fs::read_to_string(source).await.wrap_err_with(|| {
            format!(
                "failed reading submission state file at `{}`",
                source.display()
            )
        })?;
        let state: State = serde_json::from_str(&contents)
            .wrap_err_with(|| format!("failed parsing the contents of `{}`", source.display()))?;

        // Ensure the parsed values are sane.
        match &state {
            State::Fresh
            | State::Started {
                ..
            }
            | State::Finished(_) => {}
            State::Prepared {
                sequencer_height,
                last_submission,
                ..
            } => ensure!(
                *sequencer_height > last_submission.sequencer_height,
                "submission state file `{}` invalid: current sequencer height \
                 ({sequencer_height}) should be greater than last successful submission sequencer \
                 height ({})",
                source.display(),
                last_submission.sequencer_height
            ),
        }

        Ok(state)
    }

    /// Writes JSON-encoded `self` to `temp_file`, then renames `temp_file` to `destination`.
    async fn write(&self, destination: &Path, temp_file: &Path) -> eyre::Result<()> {
        let contents =
            serde_json::to_string_pretty(self).wrap_err("failed json-encoding submission state")?;
        tokio::fs::write(temp_file, &contents)
            .await
            .wrap_err_with(|| {
                format!(
                    "failed writing submission state to `{}`",
                    temp_file.display()
                )
            })?;
        tokio::fs::rename(temp_file, destination)
            .await
            .wrap_err_with(|| {
                format!(
                    "failed renaming `{}` to `{}`",
                    temp_file.display(),
                    destination.display()
                )
            })
    }
}

impl Display for State {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        if formatter.alternate() {
            write!(formatter, "{}", serde_json::to_string_pretty(self).unwrap())
        } else {
            write!(formatter, "{}", serde_json::to_string(self).unwrap())
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct SubmissionState {
    state: State,
    file_path: PathBuf,
    temp_file_path: PathBuf,
}

impl SubmissionState {
    /// Constructs a new `SubmissionState` by reading from the given `source`.
    ///
    /// `source` should be a JSON-encoded `State`, and should be writable.
    pub(super) async fn new_from_path<P: AsRef<Path>>(source: P) -> eyre::Result<Self> {
        let state = State::read(source.as_ref()).await?;
        let file_path = source.as_ref().to_path_buf();
        let temp_file_path = match file_path.extension().and_then(|extn| extn.to_str()) {
            Some(extn) => file_path.with_extension(format!("{extn}.tmp")),
            None => file_path.with_extension("tmp"),
        };

        // Ensure the state can be written.
        state
            .write(&file_path, &temp_file_path)
            .await
            .wrap_err_with(|| {
                format!(
                    "failed writing just-read submission state to disk at `{}`; is the file \
                     writable?",
                    file_path.display()
                )
            })?;

        Ok(Self {
            state,
            file_path,
            temp_file_path,
        })
    }

    /// Resets state to whatever is written to disk.
    pub(super) async fn read_from_disk(mut self) -> eyre::Result<Self> {
        let state = State::read(&self.file_path).await?;
        self.state = state;
        Ok(self)
    }

    /// Returns the sequencer height of the last completed submission, or `None` if the state is
    /// `Fresh`.
    pub(super) fn last_completed_submission(&self) -> Option<CompletedSubmission> {
        match &self.state {
            State::Fresh => None,
            State::Started {
                last_submission, ..
            }
            | State::Prepared {
                last_submission, ..
            }
            | State::Finished(last_submission) => Some(*last_submission),
        }
    }

    /// Returns the transaction hash of the last submitted `BlobTx`, and the time at which it was
    /// submitted.
    ///
    /// This will be `Some` if the state is `Prepared` (meaning that transaction may or may not be
    /// stored on Celestia and needs to be confirmed), or `None` for all other states.
    pub(super) fn tx_to_confirm(&self) -> Option<(BlobTxHash, SystemTime)> {
        if let State::Prepared {
            blob_tx_hash,
            at,
            ..
        } = &self.state
        {
            Some((*blob_tx_hash, *at))
        } else {
            None
        }
    }

    /// Sets state to `Started` regardless of the current state and writes the state to disk.
    ///
    /// Returns an error if writing fails.
    pub(super) async fn start(mut self) -> eyre::Result<Self> {
        let last_submission = match self.state {
            State::Fresh => CompletedSubmission::new(0, SequencerHeight::from(0_u8)),
            State::Started {
                last_submission, ..
            }
            | State::Prepared {
                last_submission, ..
            }
            | State::Finished(last_submission) => last_submission,
        };
        self.state = State::new_started(last_submission);

        debug!(state = %self.state, "writing submission started state to file");
        self.state
            .write(&self.file_path, &self.temp_file_path)
            .await
            .wrap_err("failed commiting submission started state to disk")?;
        Ok(self)
    }

    /// If state is `Started`, transitions to `Prepared` and writes the state to disk.
    ///
    /// Returns an error if state is anything but `Started`, if `new_sequencer_height` is not
    /// greater than the last confirmed submitted sequencer height, or if writing fails.
    pub(super) async fn prepare(
        mut self,
        new_sequencer_height: SequencerHeight,
        blob_tx_hash: BlobTxHash,
    ) -> eyre::Result<Self> {
        match self.state {
            State::Started {
                last_submission,
            } => {
                ensure!(
                    new_sequencer_height > last_submission.sequencer_height,
                    "cannot submit a sequencer block at height below or equal to what was already \
                     successfully submitted"
                );
                self.state =
                    State::new_prepared(new_sequencer_height, last_submission, blob_tx_hash);
            }
            State::Fresh
            | State::Prepared {
                ..
            }
            | State::Finished(_) => bail!("must be in started state before preparing to submit"),
        }

        debug!(state = %self.state, "writing submission prepared state to file");
        self.state
            .write(&self.file_path, &self.temp_file_path)
            .await
            .wrap_err("failed commiting submission prepared state to disk")?;
        Ok(self)
    }

    /// If state is `Prepared`, transitions to `Finished` using the current sequencer height and the
    /// provided Celestia height.
    ///
    /// Does not write the state to disk, as almost immediately the state will transition to
    /// `Started`.  Even if the process exits before that happens, the only negative effect will be
    /// that on restart, we reconfirm this submission.
    ///
    /// Returns an error if state is anything other than `Prepared`.
    pub(super) fn finish(mut self, celestia_height: u64) -> eyre::Result<Self> {
        let completed_submission = match self.state {
            State::Prepared {
                sequencer_height, ..
            } => CompletedSubmission::new(celestia_height, sequencer_height),
            State::Fresh
            | State::Started {
                ..
            }
            | State::Finished(_) => bail!("must be in prepared state before finishing submission"),
        };
        self.state = State::Finished(completed_submission);
        // No need to write state to disk as we're very soon going to be transitioning to `Started`.
        Ok(self)
    }

    /// If state is `Prepared`, reverts to `Finished` with the last known confirmed submission.
    ///
    /// Does not write the state to disk, as almost immediately the state will transition to
    /// `Started`.  Even if the process exits before that happens, the only negative effect will be
    /// that on restart, we reconfirm this submission.
    ///
    /// Returns an error if state is anything other than `Prepared`.
    pub(super) fn revert_from_prepared_to_finished(mut self) -> eyre::Result<Self> {
        match self.state {
            State::Prepared {
                last_submission, ..
            } => {
                self.state = State::Finished(last_submission);
            }
            State::Fresh
            | State::Started {
                ..
            }
            | State::Finished(_) => bail!("must be in prepared state to revert from it"),
        };
        // No need to write state to disk as we're very soon going to be transitioning to `Started`.
        Ok(self)
    }
}

impl Display for SubmissionState {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}, file_path: {}",
            self.state,
            self.file_path.display()
        )
    }
}

mod as_number {
    //! Logic to serialize sequencer heights as number, deserialize numbers as sequencer heights.
    //!
    //! This is unfortunately necessary because the [`serde::Serialize`], [`serde::Deserialize`]
    //! implementations for [`tendermint::block::Height`] write the integer as a string, probably
    //! due to tendermint's/cometbft's go-legacy.
    use serde::{
        Deserialize as _,
        Deserializer,
        Serializer,
    };

    use super::SequencerHeight;

    // Allow: the function signature is dictated by the serde(with) attribute.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(super) fn serialize<S>(height: &SequencerHeight, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(height.value())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<SequencerHeight, D::Error>
    where
        D: Deserializer<'de>,
    {
        let height = u64::deserialize(deserializer)?;
        SequencerHeight::try_from(height).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use tempfile::NamedTempFile;

    use super::*;

    const CELESTIA_HEIGHT: u64 = 1234;
    const SEQUENCER_HEIGHT_LOW: u32 = 111;
    const SEQUENCER_HEIGHT_HIGH: u32 = 222;

    #[track_caller]
    fn write(val: &serde_json::Value) -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        serde_json::to_writer(&file, val).unwrap();
        file
    }

    #[tokio::test]
    async fn should_parse_fresh() {
        let file = write(&json!({ "state": "fresh" }));
        let parsed = SubmissionState::new_from_path(file.path()).await.unwrap();
        assert_eq!(parsed.state, State::Fresh);
    }

    #[tokio::test]
    async fn should_parse_started() {
        let file = write(&json!({
            "state": "started",
            "last_submission": {
                "celestia_height": CELESTIA_HEIGHT,
                "sequencer_height": SEQUENCER_HEIGHT_LOW
            }
        }));
        let parsed = SubmissionState::new_from_path(file.path()).await.unwrap();
        let expected = State::new_started(CompletedSubmission::new(
            CELESTIA_HEIGHT,
            SequencerHeight::from(SEQUENCER_HEIGHT_LOW),
        ));
        assert_eq!(parsed.state, expected);
    }

    #[tokio::test]
    async fn should_parse_prepared() {
        const AT: &str = "2024-06-24T22:22:22.222222222Z";

        let file = write(&json!({
            "state": "prepared",
            "sequencer_height": SEQUENCER_HEIGHT_HIGH,
            "last_submission": {
                "celestia_height": CELESTIA_HEIGHT,
                "sequencer_height": SEQUENCER_HEIGHT_LOW
            },
            "blob_tx_hash": "0909090909090909090909090909090909090909090909090909090909090909",
            "at": AT
        }));
        let parsed = SubmissionState::new_from_path(file.path()).await.unwrap();
        let expected = State::Prepared {
            sequencer_height: SequencerHeight::from(SEQUENCER_HEIGHT_HIGH),
            last_submission: CompletedSubmission::new(
                CELESTIA_HEIGHT,
                SequencerHeight::from(SEQUENCER_HEIGHT_LOW),
            ),
            blob_tx_hash: BlobTxHash::from_raw([9; 32]),
            at: SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_nanos(1_719_267_742_222_222_222))
                .unwrap(),
        };
        assert_eq!(parsed.state, expected);
    }
}
