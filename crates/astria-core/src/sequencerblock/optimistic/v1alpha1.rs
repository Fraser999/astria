use bytes::Bytes;

use crate::{
    generated::astria::sequencerblock::optimistic::v1alpha1 as raw,
    sequencerblock::v1::block,
    Protobuf,
};

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SequencerFinalizedBlockInfoError(SequencerFinalizedBlockInfoErrorKind);

impl SequencerFinalizedBlockInfoError {
    fn block_hash(source: block::HashFromSliceError) -> Self {
        Self(SequencerFinalizedBlockInfoErrorKind::BlockHash {
            source,
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum SequencerFinalizedBlockInfoErrorKind {
    #[error("failed to read .block_hash field as sequencer block hash")]
    BlockHash { source: block::HashFromSliceError },
}

#[derive(Clone, Debug)]
pub struct SequencerFinalizedBlockInfo {
    height: u64,
    block_hash: block::Hash,
    pending_nonce: u32,
}

impl SequencerFinalizedBlockInfo {
    #[must_use]
    pub fn new(height: u64, block_hash: block::Hash, pending_nonce: u32) -> Self {
        Self {
            height,
            block_hash,
            pending_nonce,
        }
    }

    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }

    #[must_use]
    pub fn block_hash(&self) -> &block::Hash {
        &self.block_hash
    }

    #[must_use]
    pub fn pending_nonce(&self) -> u32 {
        self.pending_nonce
    }
}

impl From<SequencerFinalizedBlockInfo> for raw::SequencerFinalizedBlockInfo {
    fn from(value: SequencerFinalizedBlockInfo) -> Self {
        value.to_raw()
    }
}

impl Protobuf for SequencerFinalizedBlockInfo {
    type Error = SequencerFinalizedBlockInfoError;
    type Raw = raw::SequencerFinalizedBlockInfo;

    fn try_from_raw_ref(raw: &Self::Raw) -> Result<Self, Self::Error> {
        let Self::Raw {
            height,
            block_hash,
            pending_nonce,
        } = raw;

        let block_hash = block::Hash::try_from(&**block_hash)
            .map_err(SequencerFinalizedBlockInfoError::block_hash)?;

        Ok(SequencerFinalizedBlockInfo {
            height: *height,
            block_hash,
            pending_nonce: *pending_nonce,
        })
    }

    fn to_raw(&self) -> Self::Raw {
        raw::SequencerFinalizedBlockInfo {
            height: self.height(),
            block_hash: Bytes::copy_from_slice(self.block_hash.as_bytes()),
            pending_nonce: self.pending_nonce,
        }
    }
}
