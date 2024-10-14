use std::{
    borrow::Cow,
    fmt::{
        self,
        Debug,
        Formatter,
    },
};

use astria_core::sequencerblock::v1alpha1::block::{
    SequencerBlockHash,
    SEQUENCER_BLOCK_HASH_LEN,
};
use astria_eyre::eyre::bail;
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};

use super::{
    Value,
    ValueImpl,
};

#[derive(BorshSerialize, BorshDeserialize)]
pub(in crate::grpc) struct BlockHash<'a>(Cow<'a, [u8; SEQUENCER_BLOCK_HASH_LEN]>);

impl<'a> Debug for BlockHash<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0.as_slice()))
    }
}

impl<'a> From<&'a SequencerBlockHash> for BlockHash<'a> {
    fn from(block_hash: &'a SequencerBlockHash) -> Self {
        BlockHash(Cow::Borrowed(block_hash.as_bytes()))
    }
}

impl<'a> From<BlockHash<'a>> for SequencerBlockHash {
    fn from(block_hash: BlockHash<'a>) -> Self {
        SequencerBlockHash::new(block_hash.0.into_owned())
    }
}

impl<'a> From<BlockHash<'a>> for crate::storage::StoredValue<'a> {
    fn from(block_hash: BlockHash<'a>) -> Self {
        crate::storage::StoredValue::Grpc(Value(ValueImpl::BlockHash(block_hash)))
    }
}

impl<'a> TryFrom<crate::storage::StoredValue<'a>> for BlockHash<'a> {
    type Error = astria_eyre::eyre::Error;

    fn try_from(value: crate::storage::StoredValue<'a>) -> Result<Self, Self::Error> {
        let crate::storage::StoredValue::Grpc(Value(ValueImpl::BlockHash(block_hash))) = value
        else {
            bail!("grpc stored value type mismatch: expected block hash, found {value:?}");
        };
        Ok(block_hash)
    }
}
