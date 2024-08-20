use std::{
    fmt::Debug,
    io::Write,
};

use anyhow::anyhow;
use astria_core::{
    primitive::v1::{
        asset::{
            IbcPrefixed,
            TracePrefixed,
        },
        RollupId,
        ADDRESS_LEN,
    },
    sequencerblock::v1alpha1::SequencerBlock,
};
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};

use crate::authority::ValidatorSet;

pub(crate) trait Storable:
    BorshSerialize
    + BorshDeserialize
    + TryFrom<StoredValue, Error = anyhow::Error>
    + Into<StoredValue>
    + Clone
    + Debug
{
}

impl<T> Storable for T where
    T: BorshSerialize
        + BorshDeserialize
        + TryFrom<StoredValue, Error = anyhow::Error>
        + Into<StoredValue>
        + Clone
        + Debug
{
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) enum StoredValue {
    ChainId(ChainId),
    RevisionNumber(RevisionNumber),
    StorageVersion(StorageVersion),
    AddressBytes(AddressBytes),
    Balance(Balance),
    Nonce(Nonce),
    Fee(Fee),
    BasePrefix(BasePrefix),
    IbcPrefixedDenom(IbcPrefixed),
    TracePrefixedDenom(TracePrefixed),
    RollupId(RollupId),
    RollupIds(Vec<RollupId>),
    ValidatorSet(ValidatorSet),
    BlockHash(BlockHash),
    BlockHeight(BlockHeight),
    BlockTimestamp(BlockTimestamp),
    SequencerBlock(SequencerBlock),
    TxHash(TxHash),
    Unit,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct ChainId(pub(crate) String);

impl From<ChainId> for StoredValue {
    fn from(value: ChainId) -> Self {
        Self::ChainId(value)
    }
}

impl TryFrom<StoredValue> for ChainId {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::ChainId(chain_id) = value else {
            return Err(type_mismatch("ChainId", &value));
        };
        Ok(chain_id)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct RevisionNumber(pub(crate) u64);

impl From<RevisionNumber> for StoredValue {
    fn from(value: RevisionNumber) -> Self {
        Self::RevisionNumber(value)
    }
}

impl TryFrom<StoredValue> for RevisionNumber {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::RevisionNumber(revision_number) = value else {
            return Err(type_mismatch("RevisionNumber", &value));
        };
        Ok(revision_number)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct StorageVersion(pub(crate) u64);

impl From<StorageVersion> for StoredValue {
    fn from(value: StorageVersion) -> Self {
        Self::StorageVersion(value)
    }
}

impl TryFrom<StoredValue> for StorageVersion {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::StorageVersion(storage_version) = value else {
            return Err(type_mismatch("StorageVersion", &value));
        };
        Ok(storage_version)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct AddressBytes(pub(crate) [u8; ADDRESS_LEN]);

impl From<AddressBytes> for StoredValue {
    fn from(value: AddressBytes) -> Self {
        Self::AddressBytes(value)
    }
}

impl TryFrom<StoredValue> for AddressBytes {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::AddressBytes(address) = value else {
            return Err(type_mismatch("AddressBytes", &value));
        };
        Ok(address)
    }
}

#[derive(Clone, Copy, Default, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct Balance(pub(crate) u128);

impl From<Balance> for StoredValue {
    fn from(value: Balance) -> Self {
        Self::Balance(value)
    }
}

impl TryFrom<StoredValue> for Balance {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::Balance(balance) = value else {
            return Err(type_mismatch("Balance", &value));
        };
        Ok(balance)
    }
}

#[derive(Clone, Copy, Default, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct Nonce(pub(crate) u32);

impl From<Nonce> for StoredValue {
    fn from(value: Nonce) -> Self {
        Self::Nonce(value)
    }
}

impl TryFrom<StoredValue> for Nonce {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::Nonce(nonce) = value else {
            return Err(type_mismatch("Nonce", &value));
        };
        Ok(nonce)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct Fee(pub(crate) u128);

impl From<Fee> for StoredValue {
    fn from(value: Fee) -> Self {
        Self::Fee(value)
    }
}

impl TryFrom<StoredValue> for Fee {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::Fee(fee) = value else {
            return Err(type_mismatch("Fee", &value));
        };
        Ok(fee)
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct BasePrefix(pub(crate) String);

impl From<BasePrefix> for StoredValue {
    fn from(value: BasePrefix) -> Self {
        Self::BasePrefix(value)
    }
}

impl TryFrom<StoredValue> for BasePrefix {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::BasePrefix(base_prefix) = value else {
            return Err(type_mismatch("BasePrefix", &value));
        };
        Ok(base_prefix)
    }
}

impl From<IbcPrefixed> for StoredValue {
    fn from(value: IbcPrefixed) -> Self {
        Self::IbcPrefixedDenom(value)
    }
}

impl TryFrom<StoredValue> for IbcPrefixed {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::IbcPrefixedDenom(denom) = value else {
            return Err(type_mismatch("IbcPrefixedDenom", &value));
        };
        Ok(denom)
    }
}

impl From<TracePrefixed> for StoredValue {
    fn from(value: TracePrefixed) -> Self {
        Self::TracePrefixedDenom(value)
    }
}

impl TryFrom<StoredValue> for TracePrefixed {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::TracePrefixedDenom(denom) = value else {
            return Err(type_mismatch("TracePrefixedDenom", &value));
        };
        Ok(denom)
    }
}

impl From<RollupId> for StoredValue {
    fn from(value: RollupId) -> Self {
        Self::RollupId(value)
    }
}

impl TryFrom<StoredValue> for RollupId {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::RollupId(rollup_id) = value else {
            return Err(type_mismatch("RollupId", &value));
        };
        Ok(rollup_id)
    }
}

impl From<Vec<RollupId>> for StoredValue {
    fn from(value: Vec<RollupId>) -> Self {
        Self::RollupIds(value)
    }
}

impl TryFrom<StoredValue> for Vec<RollupId> {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::RollupIds(rollup_ids) = value else {
            return Err(type_mismatch("RollupIds", &value));
        };
        Ok(rollup_ids)
    }
}

impl From<ValidatorSet> for StoredValue {
    fn from(value: ValidatorSet) -> Self {
        Self::ValidatorSet(value)
    }
}

impl TryFrom<StoredValue> for ValidatorSet {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::ValidatorSet(validator_set) = value else {
            return Err(type_mismatch("ValidatorSet", &value));
        };
        Ok(validator_set)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct BlockHash(pub(crate) [u8; 32]);

impl From<BlockHash> for StoredValue {
    fn from(value: BlockHash) -> Self {
        Self::BlockHash(value)
    }
}

impl TryFrom<StoredValue> for BlockHash {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::BlockHash(block_hash) = value else {
            return Err(type_mismatch("BlockHash", &value));
        };
        Ok(block_hash)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct BlockHeight(pub(crate) u64);

impl From<BlockHeight> for StoredValue {
    fn from(value: BlockHeight) -> Self {
        Self::BlockHeight(value)
    }
}

impl TryFrom<StoredValue> for BlockHeight {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::BlockHeight(block_height) = value else {
            return Err(type_mismatch("BlockHeight", &value));
        };
        Ok(block_height)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockTimestamp(pub(crate) tendermint::time::Time);

impl From<BlockTimestamp> for StoredValue {
    fn from(value: BlockTimestamp) -> Self {
        Self::BlockTimestamp(value)
    }
}

impl TryFrom<StoredValue> for BlockTimestamp {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::BlockTimestamp(block_timestamp) = value else {
            return Err(type_mismatch("BlockTimestamp", &value));
        };
        Ok(block_timestamp)
    }
}

impl BorshSerialize for BlockTimestamp {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.0.unix_timestamp_nanos().serialize(writer)
    }
}

impl borsh::BorshDeserialize for BlockTimestamp {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let nanos = i128::deserialize_reader(reader)?;
        #[allow(clippy::cast_sign_loss)]
        let timestamp = tendermint::time::Time::from_unix_timestamp(
            i64::try_from(nanos / 1_000_000_000).unwrap(),
            (nanos % 1_000_000_000) as u32,
        )
        .map_err(std::io::Error::other)?;
        Ok(BlockTimestamp(timestamp))
    }
}

impl From<SequencerBlock> for StoredValue {
    fn from(value: SequencerBlock) -> Self {
        Self::SequencerBlock(value)
    }
}

impl TryFrom<StoredValue> for SequencerBlock {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::SequencerBlock(block) = value else {
            return Err(type_mismatch("SequencerBlock", &value));
        };
        Ok(block)
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct TxHash(pub(crate) [u8; 32]);

impl From<TxHash> for StoredValue {
    fn from(value: TxHash) -> Self {
        Self::TxHash(value)
    }
}

impl TryFrom<StoredValue> for TxHash {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::TxHash(tx_hash) = value else {
            return Err(type_mismatch("TxHash", &value));
        };
        Ok(tx_hash)
    }
}

impl From<()> for StoredValue {
    fn from((): ()) -> Self {
        Self::Unit
    }
}

impl TryFrom<StoredValue> for () {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue) -> Result<Self, Self::Error> {
        let StoredValue::Unit = value else {
            return Err(type_mismatch("Unit", &value));
        };
        Ok(())
    }
}

fn type_mismatch(expected: &'static str, found: &StoredValue) -> anyhow::Error {
    let found = match found {
        StoredValue::ChainId(_) => "ChainId",
        StoredValue::RevisionNumber(_) => "RevisionNumber",
        StoredValue::StorageVersion(_) => "StorageVersion",
        StoredValue::AddressBytes(_) => "AddressBytes",
        StoredValue::Balance(_) => "Balance",
        StoredValue::Nonce(_) => "Nonce",
        StoredValue::Fee(_) => "Fee",
        StoredValue::BasePrefix(_) => "BasePrefix",
        StoredValue::IbcPrefixedDenom(_) => "IbcPrefixedDenom",
        StoredValue::TracePrefixedDenom(_) => "TracePrefixedDenom",
        StoredValue::RollupId(_) => "RollupId",
        StoredValue::RollupIds(_) => "RollupIds",
        StoredValue::ValidatorSet(_) => "ValidatorSet",
        StoredValue::BlockHash(_) => "BlockHash",
        StoredValue::BlockHeight(_) => "BlockHeight",
        StoredValue::BlockTimestamp(_) => "BlockTimestamp",
        StoredValue::SequencerBlock(_) => "SequencerBlock",
        StoredValue::TxHash(_) => "TxHash",
        StoredValue::Unit => "Unit",
    };
    anyhow!("type mismatch: expected {expected}, found {found}")
}
