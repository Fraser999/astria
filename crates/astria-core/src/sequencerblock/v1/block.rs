use std::{
    collections::HashMap,
    fmt::Display,
    vec::IntoIter,
};

use bytes::Bytes;
use indexmap::IndexMap;
use sha2::Sha256;
use tendermint::{
    account,
    Time,
};

use super::{
    are_rollup_ids_included,
    are_rollup_txs_included,
    celestia::{
        self,
        SubmittedMetadata,
        SubmittedRollupData,
    },
    raw,
};
use crate::{
    primitive::v1::{
        asset,
        derive_merkle_tree_from_rollup_txs,
        Address,
        AddressError,
        IncorrectRollupIdLength,
        RollupId,
        TransactionId,
        TransactionIdError,
    },
    protocol::transaction::v1::{
        action,
        Transaction,
        TransactionError,
    },
    Protobuf,
};

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RollupTransactionsError(RollupTransactionsErrorKind);

impl RollupTransactionsError {
    fn rollup_id(source: IncorrectRollupIdLength) -> Self {
        Self(RollupTransactionsErrorKind::RollupId(source))
    }

    fn field_not_set(field: &'static str) -> Self {
        Self(RollupTransactionsErrorKind::FieldNotSet(field))
    }

    fn proof_invalid(source: merkle::audit::InvalidProof) -> Self {
        Self(RollupTransactionsErrorKind::ProofInvalid(source))
    }
}

#[derive(Debug, thiserror::Error)]
enum RollupTransactionsErrorKind {
    #[error("`id` field is invalid")]
    RollupId(#[source] IncorrectRollupIdLength),
    #[error("the expected field in the raw source type was not set: `{0}`")]
    FieldNotSet(&'static str),
    #[error("failed constructing a proof from the raw protobuf `proof` field")]
    ProofInvalid(#[source] merkle::audit::InvalidProof),
}

/// The individual parts that make up a [`RollupTransactions`] type.
///
/// Provides convenient access to the fields of [`RollupTransactions`].
#[derive(Clone, Debug, PartialEq)]
pub struct RollupTransactionsParts {
    pub rollup_id: RollupId,
    pub transactions: Vec<Bytes>,
    pub proof: merkle::Proof,
}

/// The opaque transactions belonging to a rollup identified by its rollup ID.
#[derive(Clone, Debug, PartialEq)]
pub struct RollupTransactions {
    /// The 32 bytes identifying a rollup. Usually the sha256 hash of a plain rollup name.
    rollup_id: RollupId,
    /// The block data for this rollup in the form of encoded [`RollupData`].
    transactions: Vec<Bytes>,
    /// Proof that this set of transactions belongs in the rollup datas merkle tree
    proof: merkle::Proof,
}

impl RollupTransactions {
    /// Returns the [`RollupId`] identifying the rollup these transactions belong to.
    #[must_use]
    pub fn rollup_id(&self) -> &RollupId {
        &self.rollup_id
    }

    /// Returns the block data for this rollup.
    #[must_use]
    pub fn transactions(&self) -> &[Bytes] {
        &self.transactions
    }

    /// Returns the merkle proof that these transactions were included
    /// in the `action_tree_commitment`.
    #[must_use]
    pub fn proof(&self) -> &merkle::Proof {
        &self.proof
    }

    /// Transforms these rollup transactions into their raw representation, which can in turn be
    /// encoded as protobuf.
    #[must_use]
    pub fn into_raw(self) -> raw::RollupTransactions {
        let Self {
            rollup_id,
            transactions,
            proof,
        } = self;
        let transactions = transactions.into_iter().map(Into::into).collect();
        raw::RollupTransactions {
            rollup_id: Some(rollup_id.into_raw()),
            transactions,
            proof: Some(proof.into_raw()),
        }
    }

    /// Attempts to transform the rollup transactions from their raw representation.
    ///
    /// # Errors
    /// Returns an error if the rollup ID bytes could not be turned into a [`RollupId`].
    pub fn try_from_raw(raw: raw::RollupTransactions) -> Result<Self, RollupTransactionsError> {
        let raw::RollupTransactions {
            rollup_id,
            transactions,
            proof,
        } = raw;
        let Some(rollup_id) = rollup_id else {
            return Err(RollupTransactionsError::field_not_set("rollup_id"));
        };
        let rollup_id =
            RollupId::try_from_raw(rollup_id).map_err(RollupTransactionsError::rollup_id)?;
        let proof = 'proof: {
            let Some(proof) = proof else {
                break 'proof Err(RollupTransactionsError::field_not_set("proof"));
            };
            merkle::Proof::try_from_raw(proof).map_err(RollupTransactionsError::proof_invalid)
        }?;
        let transactions = transactions.into_iter().map(Into::into).collect();
        Ok(Self {
            rollup_id,
            transactions,
            proof,
        })
    }

    /// Convert [`RollupTransactions`] into [`RollupTransactionsParts`].
    #[must_use]
    pub fn into_parts(self) -> RollupTransactionsParts {
        let Self {
            rollup_id,
            transactions,
            proof,
        } = self;
        RollupTransactionsParts {
            rollup_id,
            transactions,
            proof,
        }
    }

    /// This should only be used where `parts` has been provided by a trusted entity, e.g. read from
    /// our own state store.
    ///
    /// Note that this function is not considered part of the public API and is subject to breaking
    /// change at any time.
    #[cfg(feature = "unchecked-constructors")]
    #[doc(hidden)]
    #[must_use]
    pub fn unchecked_from_parts(parts: RollupTransactionsParts) -> Self {
        let RollupTransactionsParts {
            rollup_id,
            transactions,
            proof,
        } = parts;
        Self {
            rollup_id,
            transactions,
            proof,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SequencerBlockError(SequencerBlockErrorKind);

impl SequencerBlockError {
    fn invalid_block_hash(length: usize) -> Self {
        Self(SequencerBlockErrorKind::InvalidBlockHash(length))
    }

    fn field_not_set(field: &'static str) -> Self {
        Self(SequencerBlockErrorKind::FieldNotSet(field))
    }

    fn header(source: SequencerBlockHeaderError) -> Self {
        Self(SequencerBlockErrorKind::Header(source))
    }

    fn parse_rollup_transactions(source: RollupTransactionsError) -> Self {
        Self(SequencerBlockErrorKind::ParseRollupTransactions(source))
    }

    fn transaction_proof_invalid(source: merkle::audit::InvalidProof) -> Self {
        Self(SequencerBlockErrorKind::TransactionProofInvalid(source))
    }

    fn id_proof_invalid(source: merkle::audit::InvalidProof) -> Self {
        Self(SequencerBlockErrorKind::IdProofInvalid(source))
    }

    fn no_rollup_transactions_root() -> Self {
        Self(SequencerBlockErrorKind::NoRollupTransactionsRoot)
    }

    fn incorrect_rollup_transactions_root_length(len: usize) -> Self {
        Self(SequencerBlockErrorKind::IncorrectRollupTransactionsRootLength(len))
    }

    fn no_rollup_ids_root() -> Self {
        Self(SequencerBlockErrorKind::NoRollupIdsRoot)
    }

    fn incorrect_rollup_ids_root_length(len: usize) -> Self {
        Self(SequencerBlockErrorKind::IncorrectRollupIdsRootLength(len))
    }

    fn rollup_transactions_not_in_sequencer_block() -> Self {
        Self(SequencerBlockErrorKind::RollupTransactionsNotInSequencerBlock)
    }

    fn rollup_ids_not_in_sequencer_block() -> Self {
        Self(SequencerBlockErrorKind::RollupIdsNotInSequencerBlock)
    }

    fn transaction_protobuf_decode(source: prost::DecodeError) -> Self {
        Self(SequencerBlockErrorKind::TransactionProtobufDecode(source))
    }

    fn raw_signed_transaction_conversion(source: TransactionError) -> Self {
        Self(SequencerBlockErrorKind::RawTransactionConversion(source))
    }

    fn rollup_transactions_root_does_not_match_reconstructed() -> Self {
        Self(SequencerBlockErrorKind::RollupTransactionsRootDoesNotMatchReconstructed)
    }

    fn rollup_ids_root_does_not_match_reconstructed() -> Self {
        Self(SequencerBlockErrorKind::RollupIdsRootDoesNotMatchReconstructed)
    }

    fn invalid_rollup_transactions_root() -> Self {
        Self(SequencerBlockErrorKind::InvalidRollupTransactionsRoot)
    }

    fn invalid_rollup_ids_proof() -> Self {
        Self(SequencerBlockErrorKind::InvalidRollupIdsProof)
    }
}

#[derive(Debug, thiserror::Error)]
enum SequencerBlockErrorKind {
    #[error("the block hash was expected to be 32 bytes long, but was actually `{0}`")]
    InvalidBlockHash(usize),
    #[error("the expected field in the raw source type was not set: `{0}`")]
    FieldNotSet(&'static str),
    #[error("failed constructing a sequencer block header from the raw protobuf header")]
    Header(#[source] SequencerBlockHeaderError),
    #[error(
        "failed parsing a raw protobuf rollup transaction because it contained an invalid rollup \
         ID"
    )]
    ParseRollupTransactions(#[source] RollupTransactionsError),
    #[error("failed constructing a transaction proof from the raw protobuf transaction proof")]
    TransactionProofInvalid(#[source] merkle::audit::InvalidProof),
    #[error("failed constructing a rollup ID proof from the raw protobuf rollup ID proof")]
    IdProofInvalid(#[source] merkle::audit::InvalidProof),
    #[error(
        "the cometbft block.data field was too short and did not contain the rollup transaction \
         root"
    )]
    NoRollupTransactionsRoot,
    #[error(
        "the rollup transaction root in the cometbft block.data field was expected to be 32 bytes \
         long, but was actually `{0}`"
    )]
    IncorrectRollupTransactionsRootLength(usize),
    #[error("the cometbft block.data field was too short and did not contain the rollup ID root")]
    NoRollupIdsRoot,
    #[error(
        "the rollup ID root in the cometbft block.data field was expected to be 32 bytes long, \
         but was actually `{0}`"
    )]
    IncorrectRollupIdsRootLength(usize),
    #[error(
        "the Merkle Tree Hash derived from the rollup transactions recorded in the raw protobuf \
         sequencer block could not be verified against their proof and the block's data hash"
    )]
    RollupTransactionsNotInSequencerBlock,
    #[error(
        "the Merkle Tree Hash derived from the rollup IDs recorded in the raw protobuf sequencer \
         block could not be verified against their proof and the block's data hash"
    )]
    RollupIdsNotInSequencerBlock,
    #[error(
        "failed decoding an entry in the cometbft block.data field as a protobuf astria \
         transaction"
    )]
    TransactionProtobufDecode(#[source] prost::DecodeError),
    #[error(
        "failed converting a raw protobuf transaction decoded from the cometbft block.data
        field to a native astria transaction"
    )]
    RawTransactionConversion(#[source] TransactionError),
    #[error(
        "the root derived from the rollup transactions in the cometbft block.data field did not \
         match the root stored in the same block.data field"
    )]
    RollupTransactionsRootDoesNotMatchReconstructed,
    #[error(
        "the root derived from the rollup IDs in the cometbft block.data field did not match the \
         root stored in the same block.data field"
    )]
    RollupIdsRootDoesNotMatchReconstructed,
    #[error(
        "the rollup transactions root in the header did not verify against data_hash given the \
         rollup transactions proof"
    )]
    InvalidRollupTransactionsRoot,
    #[error(
        "the rollup IDs root constructed from the block's rollup IDs did not verify against \
         data_hash given the rollup IDs proof"
    )]
    InvalidRollupIdsProof,
}

/// The individual parts that make up a [`SequencerBlockHeader`].
///
/// This type exists to provide convenient access to the fields of
/// a `[SequencerBlockHeader]`.
#[derive(Debug, PartialEq)]
pub struct SequencerBlockHeaderParts {
    pub chain_id: tendermint::chain::Id,
    pub height: tendermint::block::Height,
    pub time: Time,
    pub rollup_transactions_root: [u8; 32],
    pub data_hash: [u8; 32],
    pub proposer_address: account::Id,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequencerBlockHeader {
    chain_id: tendermint::chain::Id,
    height: tendermint::block::Height,
    time: Time,
    // the 32-byte merkle root of all the rollup transactions in the block
    rollup_transactions_root: [u8; 32],
    data_hash: [u8; 32],
    proposer_address: account::Id,
}

impl SequencerBlockHeader {
    #[must_use]
    pub fn chain_id(&self) -> &tendermint::chain::Id {
        &self.chain_id
    }

    #[must_use]
    pub fn height(&self) -> tendermint::block::Height {
        self.height
    }

    #[must_use]
    pub fn time(&self) -> Time {
        self.time
    }

    #[must_use]
    pub fn rollup_transactions_root(&self) -> &[u8; 32] {
        &self.rollup_transactions_root
    }

    #[must_use]
    pub fn data_hash(&self) -> &[u8; 32] {
        &self.data_hash
    }

    #[must_use]
    pub fn proposer_address(&self) -> &account::Id {
        &self.proposer_address
    }

    /// Convert [`SequencerBlockHeader`] into its [`SequencerBlockHeaderParts`].
    #[must_use]
    pub fn into_parts(self) -> SequencerBlockHeaderParts {
        let Self {
            chain_id,
            height,
            time,
            rollup_transactions_root,
            data_hash,
            proposer_address,
        } = self;
        SequencerBlockHeaderParts {
            chain_id,
            height,
            time,
            rollup_transactions_root,
            data_hash,
            proposer_address,
        }
    }

    #[must_use]
    pub fn into_raw(self) -> raw::SequencerBlockHeader {
        let time: tendermint_proto::google::protobuf::Timestamp = self.time.into();
        raw::SequencerBlockHeader {
            chain_id: self.chain_id.to_string(),
            height: self.height.value(),
            time: Some(pbjson_types::Timestamp {
                seconds: time.seconds,
                nanos: time.nanos,
            }),
            rollup_transactions_root: Bytes::copy_from_slice(&self.rollup_transactions_root),
            data_hash: Bytes::copy_from_slice(&self.data_hash),
            proposer_address: Bytes::copy_from_slice(self.proposer_address.as_bytes()),
        }
    }

    /// Attempts to transform the sequencer block header from its raw representation.
    ///
    /// # Errors
    ///
    /// - If the `cometbft_header` field is not set.
    /// - If the `cometbft_header` field cannot be converted.
    /// - If the `rollup_transactions_root` field is not 32 bytes long.
    pub fn try_from_raw(raw: raw::SequencerBlockHeader) -> Result<Self, SequencerBlockHeaderError> {
        let raw::SequencerBlockHeader {
            chain_id,
            height,
            time,
            rollup_transactions_root,
            data_hash,
            proposer_address,
            ..
        } = raw;

        let chain_id = tendermint::chain::Id::try_from(chain_id)
            .map_err(SequencerBlockHeaderError::invalid_chain_id)?;

        let height = tendermint::block::Height::try_from(height)
            .map_err(SequencerBlockHeaderError::invalid_height)?;

        let Some(time) = time else {
            return Err(SequencerBlockHeaderError::field_not_set("time"));
        };
        let time = Time::try_from(tendermint_proto::google::protobuf::Timestamp {
            seconds: time.seconds,
            nanos: time.nanos,
        })
        .map_err(SequencerBlockHeaderError::time)?;

        let rollup_transactions_root =
            rollup_transactions_root.as_ref().try_into().map_err(|_| {
                SequencerBlockHeaderError::incorrect_rollup_transactions_root_length(
                    rollup_transactions_root.len(),
                )
            })?;

        let data_hash = data_hash.as_ref().try_into().map_err(|_| {
            SequencerBlockHeaderError::incorrect_rollup_transactions_root_length(data_hash.len())
        })?;

        let proposer_address = account::Id::try_from(proposer_address)
            .map_err(SequencerBlockHeaderError::proposer_address)?;

        Ok(Self {
            chain_id,
            height,
            time,
            rollup_transactions_root,
            data_hash,
            proposer_address,
        })
    }

    /// This should only be used where `parts` has been provided by a trusted entity, e.g. read from
    /// our own state store.
    ///
    /// Note that this function is not considered part of the public API and is subject to breaking
    /// change at any time.
    #[cfg(feature = "unchecked-constructors")]
    #[doc(hidden)]
    #[must_use]
    pub fn unchecked_from_parts(parts: SequencerBlockHeaderParts) -> Self {
        let SequencerBlockHeaderParts {
            chain_id,
            height,
            time,
            rollup_transactions_root,
            data_hash,
            proposer_address,
        } = parts;
        Self {
            chain_id,
            height,
            time,
            rollup_transactions_root,
            data_hash,
            proposer_address,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SequencerBlockHeaderError(SequencerBlockHeaderErrorKind);

impl SequencerBlockHeaderError {
    fn invalid_chain_id(source: tendermint::Error) -> Self {
        Self(SequencerBlockHeaderErrorKind::InvalidChainId(source))
    }

    fn invalid_height(source: tendermint::Error) -> Self {
        Self(SequencerBlockHeaderErrorKind::InvalidHeight(source))
    }

    fn field_not_set(field: &'static str) -> Self {
        Self(SequencerBlockHeaderErrorKind::FieldNotSet(field))
    }

    fn time(source: tendermint::Error) -> Self {
        Self(SequencerBlockHeaderErrorKind::Time(source))
    }

    fn incorrect_rollup_transactions_root_length(len: usize) -> Self {
        Self(SequencerBlockHeaderErrorKind::IncorrectRollupTransactionsRootLength(len))
    }

    fn proposer_address(source: tendermint::Error) -> Self {
        Self(SequencerBlockHeaderErrorKind::ProposerAddress(source))
    }
}

#[derive(Debug, thiserror::Error)]
enum SequencerBlockHeaderErrorKind {
    #[error("the chain ID in the raw protobuf sequencer block header was invalid")]
    InvalidChainId(#[source] tendermint::Error),
    #[error("the height in the raw protobuf sequencer block header was invalid")]
    InvalidHeight(#[source] tendermint::Error),
    #[error("the expected field in the raw source type was not set: `{0}`")]
    FieldNotSet(&'static str),
    #[error("failed to create a tendermint time from the raw protobuf time")]
    Time(#[source] tendermint::Error),
    #[error(
        "the rollup transaction root in the cometbft block.data field was expected to be 32 bytes \
         long, but was actually `{0}`"
    )]
    IncorrectRollupTransactionsRootLength(usize),
    #[error(
        "the proposer address in the raw protobuf sequencer block header was not 20 bytes long"
    )]
    ProposerAddress(#[source] tendermint::Error),
}

/// The individual parts that make up a [`SequencerBlock`].
///
/// Exists to provide convenient access to fields of a [`SequencerBlock`].
#[derive(Clone, Debug, PartialEq)]
pub struct SequencerBlockParts {
    pub block_hash: Hash,
    pub header: SequencerBlockHeader,
    pub rollup_transactions: IndexMap<RollupId, RollupTransactions>,
    pub rollup_transactions_proof: merkle::Proof,
    pub rollup_ids_proof: merkle::Proof,
}

/// A newtype wrapper around `[u8; 32]` to represent the hash of a [`SequencerBlock`].
///
/// [`Hash`] is the cometbft constructed hash of block.
///
/// There are two main purposes of this type:
///
/// 1. avoid confusion with other hashes of the form `[u8; 32]` common in Astria, like rollup
///    (ethereum) 32 byte hashes.
/// 2. to provide a hex formatted display impl, which is the convention for block hashes.
///
/// Note that hex based [`Display`] impl of [`Hash`] does not follow the pbjson
/// convention to display protobuf `bytes` using base64 encoding. To get the
/// display formatting faithful to pbjson convention use the alternative formatting selector,
/// `{block_hash:#}` instead.
///
/// # Examples
///
/// ```
/// use astria_core::sequencerblock::v1::block;
///
/// let block_hash = block::Hash::new([42; 32]);
/// assert_eq!(
///     "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a",
///     format!("{block_hash}"),
/// );
/// assert_eq!(
///     "KioqKioqKioqKioqKioqKioqKioqKioqKioqKioqKio=",
///     format!("{block_hash:#}"),
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; 32]);

impl Hash {
    #[must_use]
    pub const fn new(inner: [u8; 32]) -> Self {
        Self(inner)
    }

    #[must_use]
    pub const fn get(self) -> [u8; 32] {
        self.0
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("block hash requires 32 bytes, but slice contained `{actual}`")]
pub struct HashFromSliceError {
    actual: usize,
    source: std::array::TryFromSliceError,
}

impl<'a> TryFrom<&'a [u8]> for Hash {
    type Error = HashFromSliceError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        let inner = value.try_into().map_err(|source| Self::Error {
            actual: value.len(),
            source,
        })?;
        Ok(Self(inner))
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::{
            display::Base64Display,
            engine::general_purpose::STANDARD,
        };

        if f.alternate() {
            Base64Display::new(&self.0, &STANDARD).fmt(f)?;
        } else {
            for byte in self.0 {
                write!(f, "{byte:02x}")?;
            }
        }
        Ok(())
    }
}

/// `SequencerBlock` is constructed from a tendermint/cometbft block by
/// converting its opaque `data` bytes into sequencer specific types.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "we want consistent and specific naming"
)]
pub struct SequencerBlock {
    /// The result of hashing the cometbft header. Guaranteed to not be `None` as compared to
    /// the cometbft/tendermint-rs return type.
    block_hash: Hash,
    /// the block header, which contains the cometbft header and additional sequencer-specific
    /// commitments.
    header: SequencerBlockHeader,
    /// The collection of rollup transactions that were included in this block.
    rollup_transactions: IndexMap<RollupId, RollupTransactions>,
    /// The proof that the rollup transactions are included in the `CometBFT` block this
    /// sequencer block is derived form. This proof together with
    /// `Sha256(MTH(rollup_transactions))` must match `header.data_hash`.
    /// `MTH(rollup_transactions)` is the Merkle Tree Hash derived from the
    /// rollup transactions.
    rollup_transactions_proof: merkle::Proof,
    /// The proof that the rollup IDs listed in `rollup_transactions` are included
    /// in the `CometBFT` block this sequencer block is derived form. This proof together
    /// with `Sha256(MTH(rollup_ids))` must match `header.data_hash`.
    /// `MTH(rollup_ids)` is the Merkle Tree Hash derived from the rollup IDs listed in
    /// the rollup transactions.
    rollup_ids_proof: merkle::Proof,
}

impl SequencerBlock {
    /// Returns the hash of the `CometBFT` block this sequencer block is derived from.
    ///
    /// This is done by hashing the `CometBFT` header stored in this block.
    #[must_use]
    pub fn block_hash(&self) -> &Hash {
        &self.block_hash
    }

    #[must_use]
    pub fn header(&self) -> &SequencerBlockHeader {
        &self.header
    }

    /// The height stored in this sequencer block.
    #[must_use]
    pub fn height(&self) -> tendermint::block::Height {
        self.header.height
    }

    #[must_use]
    pub fn rollup_transactions(&self) -> &IndexMap<RollupId, RollupTransactions> {
        &self.rollup_transactions
    }

    #[must_use]
    pub fn rollup_transactions_proof(&self) -> &merkle::Proof {
        &self.rollup_transactions_proof
    }

    #[must_use]
    pub fn rollup_ids_proof(&self) -> &merkle::Proof {
        &self.rollup_ids_proof
    }

    /// Converts a [`SequencerBlock`] into its [`SequencerBlockParts`].
    #[must_use]
    pub fn into_parts(self) -> SequencerBlockParts {
        let Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        } = self;
        SequencerBlockParts {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        }
    }

    /// Returns the map of rollup transactions, consuming `self`.
    #[must_use]
    pub fn into_rollup_transactions(self) -> IndexMap<RollupId, RollupTransactions> {
        self.rollup_transactions
    }

    #[must_use]
    pub fn into_raw(self) -> raw::SequencerBlock {
        let Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        } = self;
        raw::SequencerBlock {
            block_hash: Bytes::copy_from_slice(block_hash.as_bytes()),
            header: Some(header.into_raw()),
            rollup_transactions: rollup_transactions
                .into_values()
                .map(RollupTransactions::into_raw)
                .collect(),
            rollup_transactions_proof: Some(rollup_transactions_proof.into_raw()),
            rollup_ids_proof: Some(rollup_ids_proof.into_raw()),
        }
    }

    #[must_use]
    pub fn into_filtered_block<I, R>(mut self, rollup_ids: I) -> FilteredSequencerBlock
    where
        I: IntoIterator<Item = R>,
        RollupId: From<R>,
    {
        let all_rollup_ids: Vec<RollupId> = self.rollup_transactions.keys().copied().collect();

        let mut filtered_rollup_transactions = IndexMap::new();
        for id in rollup_ids {
            let id = id.into();
            if let Some(rollup_transactions) = self.rollup_transactions.shift_remove(&id) {
                filtered_rollup_transactions.insert(id, rollup_transactions);
            };
        }

        FilteredSequencerBlock {
            block_hash: self.block_hash,
            header: self.header,
            rollup_transactions: filtered_rollup_transactions,
            rollup_transactions_proof: self.rollup_transactions_proof,
            all_rollup_ids,
            rollup_ids_proof: self.rollup_ids_proof,
        }
    }

    #[must_use]
    pub fn to_filtered_block<I, R>(&self, rollup_ids: I) -> FilteredSequencerBlock
    where
        I: IntoIterator<Item = R>,
        RollupId: From<R>,
    {
        let all_rollup_ids: Vec<RollupId> = self.rollup_transactions.keys().copied().collect();

        let mut filtered_rollup_transactions = IndexMap::new();
        for id in rollup_ids {
            let id = id.into();
            if let Some(rollup_transactions) = self.rollup_transactions.get(&id).cloned() {
                filtered_rollup_transactions.insert(id, rollup_transactions);
            };
        }

        FilteredSequencerBlock {
            block_hash: self.block_hash,
            header: self.header.clone(),
            rollup_transactions: filtered_rollup_transactions,
            rollup_transactions_proof: self.rollup_transactions_proof.clone(),
            all_rollup_ids,
            rollup_ids_proof: self.rollup_ids_proof.clone(),
        }
    }

    /// Turn the sequencer block into a [`SubmittedMetadata`] and list of [`SubmittedRollupData`].
    #[must_use]
    pub fn split_for_celestia(self) -> (SubmittedMetadata, Vec<SubmittedRollupData>) {
        celestia::PreparedBlock::from_sequencer_block(self).into_parts()
    }

    /// Converts from relevant header fields and the block data.
    ///
    /// # Errors
    /// TODO(https://github.com/astriaorg/astria/issues/612)
    ///
    /// # Panics
    ///
    /// - if a rollup data merkle proof cannot be constructed.
    pub fn try_from_block_info_and_data(
        block_hash: [u8; 32],
        chain_id: tendermint::chain::Id,
        height: tendermint::block::Height,
        time: Time,
        proposer_address: account::Id,
        data: Vec<Bytes>,
        deposits: HashMap<RollupId, Vec<Deposit>>,
    ) -> Result<Self, SequencerBlockError> {
        use prost::Message as _;

        let tree = merkle_tree_from_data(&data);
        let data_hash = tree.root();

        let mut data_list = data.into_iter();
        let (rollup_transactions_root, rollup_ids_root) =
            rollup_transactions_and_ids_root_from_data(&mut data_list)?;

        let mut rollup_datas = IndexMap::new();
        for elem in data_list {
            let raw_tx =
                crate::generated::astria::protocol::transaction::v1::Transaction::decode(&*elem)
                    .map_err(SequencerBlockError::transaction_protobuf_decode)?;
            let tx = Transaction::try_from_raw(raw_tx)
                .map_err(SequencerBlockError::raw_signed_transaction_conversion)?;
            for action in tx.into_unsigned().into_actions() {
                // XXX: The fee asset is dropped. We shjould explain why that's ok.
                if let action::Action::RollupDataSubmission(action::RollupDataSubmission {
                    rollup_id,
                    data,
                    fee_asset: _,
                }) = action
                {
                    let elem = rollup_datas.entry(rollup_id).or_insert(vec![]);
                    let data = RollupData::SequencedData(data)
                        .into_raw()
                        .encode_to_vec()
                        .into();
                    elem.push(data);
                }
            }
        }
        for (id, deposits) in deposits {
            rollup_datas
                .entry(id)
                .or_default()
                .extend(deposits.into_iter().map(|deposit| {
                    RollupData::Deposit(Box::new(deposit))
                        .into_raw()
                        .encode_to_vec()
                        .into()
                }));
        }

        // XXX: The rollup data must be sorted by its keys before constructing the merkle tree.
        // Since it's constructed from non-deterministically ordered sources, there is otherwise no
        // guarantee that the same data will give the root.
        rollup_datas.sort_unstable_keys();

        // ensure the rollup IDs commitment matches the one calculated from the rollup data
        if rollup_ids_root != merkle::Tree::from_leaves(rollup_datas.keys()).root() {
            return Err(SequencerBlockError::rollup_ids_root_does_not_match_reconstructed());
        }

        let rollup_transaction_tree = derive_merkle_tree_from_rollup_txs(&rollup_datas);
        if rollup_transactions_root != rollup_transaction_tree.root() {
            return Err(
                SequencerBlockError::rollup_transactions_root_does_not_match_reconstructed(),
            );
        }

        let mut rollup_transactions = IndexMap::new();
        for (i, (rollup_id, data)) in rollup_datas.into_iter().enumerate() {
            let proof = rollup_transaction_tree
                .construct_proof(i)
                .expect("the proof must exist because the tree was derived with the same leaf");
            rollup_transactions.insert(
                rollup_id,
                RollupTransactions {
                    rollup_id,
                    transactions: data, // TODO: rename this field?
                    proof,
                },
            );
        }
        rollup_transactions.sort_unstable_keys();

        // action tree root is always the first tx in a block
        let rollup_transactions_proof = tree.construct_proof(0).expect(
            "the tree has at least one leaf; if this line is reached and `construct_proof` \
             returns None it means that the short circuiting checks above it have been removed",
        );

        let rollup_ids_proof = tree.construct_proof(1).expect(
            "the tree has at least two leaves; if this line is reached and `construct_proof` \
             returns None it means that the short circuiting checks above it have been removed",
        );

        Ok(Self {
            block_hash: Hash(block_hash),
            header: SequencerBlockHeader {
                chain_id,
                height,
                time,
                rollup_transactions_root,
                data_hash,
                proposer_address,
            },
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        })
    }

    /// Converts from the raw decoded protobuf representation of this type.
    ///
    /// # Errors
    /// TODO(https://github.com/astriaorg/astria/issues/612)
    pub fn try_from_raw(raw: raw::SequencerBlock) -> Result<Self, SequencerBlockError> {
        use sha2::Digest as _;

        fn rollup_txs_to_tuple(
            raw: raw::RollupTransactions,
        ) -> Result<(RollupId, RollupTransactions), RollupTransactionsError> {
            let rollup_transactions = RollupTransactions::try_from_raw(raw)?;
            Ok((rollup_transactions.rollup_id, rollup_transactions))
        }

        let raw::SequencerBlock {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        } = raw;

        let block_hash = block_hash
            .as_ref()
            .try_into()
            .map_err(|_| SequencerBlockError::invalid_block_hash(block_hash.len()))?;

        let rollup_transactions_proof = 'proof: {
            let Some(rollup_transactions_proof) = rollup_transactions_proof else {
                break 'proof Err(SequencerBlockError::field_not_set(
                    "rollup_transactions_proof",
                ));
            };
            merkle::Proof::try_from_raw(rollup_transactions_proof)
                .map_err(SequencerBlockError::transaction_proof_invalid)
        }?;
        let rollup_ids_proof = 'proof: {
            let Some(rollup_ids_proof) = rollup_ids_proof else {
                break 'proof Err(SequencerBlockError::field_not_set("rollup_ids_proof"));
            };
            merkle::Proof::try_from_raw(rollup_ids_proof)
                .map_err(SequencerBlockError::id_proof_invalid)
        }?;
        let header = 'header: {
            let Some(header) = header else {
                break 'header Err(SequencerBlockError::field_not_set("header"));
            };
            SequencerBlockHeader::try_from_raw(header).map_err(SequencerBlockError::header)
        }?;

        let rollup_transactions: IndexMap<RollupId, RollupTransactions> = rollup_transactions
            .into_iter()
            .map(rollup_txs_to_tuple)
            .collect::<Result<_, _>>()
            .map_err(SequencerBlockError::parse_rollup_transactions)?;

        let data_hash = header.data_hash;

        if !rollup_transactions_proof
            .verify(&Sha256::digest(header.rollup_transactions_root), data_hash)
        {
            return Err(SequencerBlockError::invalid_rollup_transactions_root());
        };

        let rollup_ids_root = merkle::Tree::from_leaves(rollup_transactions.keys()).root();
        if !rollup_ids_proof.verify(&Sha256::digest(rollup_ids_root), data_hash) {
            return Err(SequencerBlockError::invalid_rollup_ids_proof());
        };

        if !are_rollup_txs_included(&rollup_transactions, &rollup_transactions_proof, data_hash) {
            return Err(SequencerBlockError::rollup_transactions_not_in_sequencer_block());
        }
        if !are_rollup_ids_included(
            rollup_transactions.keys().copied(),
            &rollup_ids_proof,
            data_hash,
        ) {
            return Err(SequencerBlockError::rollup_ids_not_in_sequencer_block());
        }

        Ok(Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        })
    }

    /// This should only be used where `parts` has been provided by a trusted entity, e.g. read from
    /// our own state store.
    ///
    /// Note that this function is not considered part of the public API and is subject to breaking
    /// change at any time.
    #[cfg(feature = "unchecked-constructors")]
    #[doc(hidden)]
    #[must_use]
    pub fn unchecked_from_parts(parts: SequencerBlockParts) -> Self {
        let SequencerBlockParts {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        } = parts;
        Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
        }
    }
}

fn rollup_transactions_and_ids_root_from_data(
    data_list: &mut IntoIter<Bytes>,
) -> Result<([u8; 32], [u8; 32]), SequencerBlockError> {
    let rollup_transactions_root: [u8; 32] = data_list
        .next()
        .ok_or(SequencerBlockError::no_rollup_transactions_root())?
        .as_ref()
        .try_into()
        .map_err(|_| {
            SequencerBlockError::incorrect_rollup_transactions_root_length(data_list.len())
        })?;
    let rollup_ids_root: [u8; 32] = data_list
        .next()
        .ok_or(SequencerBlockError::no_rollup_ids_root())?
        .as_ref()
        .try_into()
        .map_err(|_| SequencerBlockError::incorrect_rollup_ids_root_length(data_list.len()))?;
    Ok((rollup_transactions_root, rollup_ids_root))
}

/// Constructs a `[merkle::Tree]` from an iterator yielding byte slices.
///
/// This hashes each item before pushing it into the Merkle Tree, which
/// effectively causes a double hashing. The leaf hash of an item `d_i`
/// is then `MTH(d_i) = SHA256(0x00 || SHA256(d_i))`.
pub fn merkle_tree_from_data<I, B>(iter: I) -> merkle::Tree
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    use sha2::Digest as _;
    merkle::Tree::from_leaves(iter.into_iter().map(|item| Sha256::digest(&item)))
}

/// The individual parts that make up a [`FilteredSequencerBlock`].
///
/// Exists to provide convenient access to fields of a [`FilteredSequencerBlock`].
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredSequencerBlockParts {
    pub block_hash: Hash,
    pub header: SequencerBlockHeader,
    // filtered set of rollup transactions
    pub rollup_transactions: IndexMap<RollupId, RollupTransactions>,
    // proof that `rollup_transactions_root` is included in `data_hash`
    pub rollup_transactions_proof: merkle::Proof,
    // all rollup ids in the sequencer block
    pub all_rollup_ids: Vec<RollupId>,
    // proof that `rollup_ids` is included in `data_hash`
    pub rollup_ids_proof: merkle::Proof,
}

#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "we want consistent and specific naming"
)]
pub struct FilteredSequencerBlock {
    block_hash: Hash,
    header: SequencerBlockHeader,
    // filtered set of rollup transactions
    rollup_transactions: IndexMap<RollupId, RollupTransactions>,
    // proof that `rollup_transactions_root` is included in `data_hash`
    rollup_transactions_proof: merkle::Proof,
    // all rollup ids in the sequencer block
    all_rollup_ids: Vec<RollupId>,
    // proof that `rollup_ids` is included in `data_hash`
    rollup_ids_proof: merkle::Proof,
}

impl FilteredSequencerBlock {
    #[must_use]
    pub fn block_hash(&self) -> &Hash {
        &self.block_hash
    }

    #[must_use]
    pub fn header(&self) -> &SequencerBlockHeader {
        &self.header
    }

    #[must_use]
    pub fn height(&self) -> tendermint::block::Height {
        self.header.height
    }

    #[must_use]
    pub fn rollup_transactions(&self) -> &IndexMap<RollupId, RollupTransactions> {
        &self.rollup_transactions
    }

    #[must_use]
    pub fn rollup_transactions_root(&self) -> &[u8; 32] {
        &self.header.rollup_transactions_root
    }

    #[must_use]
    pub fn rollup_transactions_proof(&self) -> &merkle::Proof {
        &self.rollup_transactions_proof
    }

    #[must_use]
    pub fn all_rollup_ids(&self) -> &[RollupId] {
        &self.all_rollup_ids
    }

    #[must_use]
    pub fn rollup_ids_proof(&self) -> &merkle::Proof {
        &self.rollup_ids_proof
    }

    #[must_use]
    pub fn into_raw(self) -> raw::FilteredSequencerBlock {
        let Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            rollup_ids_proof,
            ..
        } = self;
        raw::FilteredSequencerBlock {
            block_hash: Bytes::copy_from_slice(block_hash.as_bytes()),
            header: Some(header.into_raw()),
            rollup_transactions: rollup_transactions
                .into_values()
                .map(RollupTransactions::into_raw)
                .collect(),
            rollup_transactions_proof: Some(rollup_transactions_proof.into_raw()),
            all_rollup_ids: self.all_rollup_ids.iter().map(RollupId::to_raw).collect(),
            rollup_ids_proof: Some(rollup_ids_proof.into_raw()),
        }
    }

    /// Converts from the raw decoded protobuf representation of this type.
    ///
    /// # Errors
    ///
    /// - if the rollup transactions proof is not set
    /// - if the rollup IDs proof is not set
    /// - if the rollup transactions proof cannot be constructed from the raw protobuf
    /// - if the rollup IDs proof cannot be constructed from the raw protobuf
    /// - if the cometbft header is not set
    /// - if the cometbft header cannot be constructed from the raw protobuf
    /// - if the cometbft block hash is None
    /// - if the data hash is None
    /// - if the rollup transactions cannot be parsed
    /// - if the rollup transactions root is not 32 bytes
    /// - if the rollup transactions are not included in the sequencer block
    /// - if the rollup IDs root is not 32 bytes
    /// - if the rollup IDs are not included in the sequencer block
    pub fn try_from_raw(
        raw: raw::FilteredSequencerBlock,
    ) -> Result<Self, FilteredSequencerBlockError> {
        use sha2::Digest as _;

        fn rollup_txs_to_tuple(
            raw: raw::RollupTransactions,
        ) -> Result<(RollupId, RollupTransactions), RollupTransactionsError> {
            let rollup_transactions = RollupTransactions::try_from_raw(raw)?;
            Ok((rollup_transactions.rollup_id, rollup_transactions))
        }

        let raw::FilteredSequencerBlock {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            all_rollup_ids,
            rollup_ids_proof,
            ..
        } = raw;

        let block_hash = block_hash
            .as_ref()
            .try_into()
            .map_err(|_| FilteredSequencerBlockError::invalid_block_hash(block_hash.len()))?;

        let rollup_transactions_proof = {
            let Some(rollup_transactions_proof) = rollup_transactions_proof else {
                return Err(FilteredSequencerBlockError::field_not_set(
                    "rollup_transactions_proof",
                ));
            };
            merkle::Proof::try_from_raw(rollup_transactions_proof)
                .map_err(FilteredSequencerBlockError::transaction_proof_invalid)
        }?;
        let rollup_ids_proof = {
            let Some(rollup_ids_proof) = rollup_ids_proof else {
                return Err(FilteredSequencerBlockError::field_not_set(
                    "rollup_ids_proof",
                ));
            };
            merkle::Proof::try_from_raw(rollup_ids_proof)
                .map_err(FilteredSequencerBlockError::id_proof_invalid)
        }?;
        let header = {
            let Some(header) = header else {
                return Err(FilteredSequencerBlockError::field_not_set("header"));
            };
            SequencerBlockHeader::try_from_raw(header)
                .map_err(FilteredSequencerBlockError::invalid_header)
        }?;

        // XXX: These rollup transactions are not sorted compared to those used for
        // deriving the rollup transactions merkle tree in `SequencerBlock`.
        let rollup_transactions = rollup_transactions
            .into_iter()
            .map(rollup_txs_to_tuple)
            .collect::<Result<IndexMap<_, _>, _>>()
            .map_err(FilteredSequencerBlockError::parse_rollup_transactions)?;

        let all_rollup_ids: Vec<RollupId> = all_rollup_ids
            .into_iter()
            .map(RollupId::try_from_raw)
            .collect::<Result<_, _>>()
            .map_err(FilteredSequencerBlockError::invalid_rollup_id)?;

        if !rollup_transactions_proof.verify(
            &Sha256::digest(header.rollup_transactions_root),
            header.data_hash,
        ) {
            return Err(FilteredSequencerBlockError::rollup_transactions_not_in_sequencer_block());
        }

        for rollup_transactions in rollup_transactions.values() {
            if !super::do_rollup_transaction_match_root(
                rollup_transactions,
                header.rollup_transactions_root,
            ) {
                return Err(
                    FilteredSequencerBlockError::rollup_transaction_for_id_not_in_sequencer_block(
                        *rollup_transactions.rollup_id(),
                    ),
                );
            }
        }

        if !are_rollup_ids_included(
            all_rollup_ids.iter().copied(),
            &rollup_ids_proof,
            header.data_hash,
        ) {
            return Err(FilteredSequencerBlockError::rollup_ids_not_in_sequencer_block());
        }

        Ok(Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            all_rollup_ids,
            rollup_ids_proof,
        })
    }

    /// Transforms the filtered blocks into its constituent parts.
    #[must_use]
    pub fn into_parts(self) -> FilteredSequencerBlockParts {
        let Self {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            all_rollup_ids,
            rollup_ids_proof,
        } = self;
        FilteredSequencerBlockParts {
            block_hash,
            header,
            rollup_transactions,
            rollup_transactions_proof,
            all_rollup_ids,
            rollup_ids_proof,
        }
    }
}

impl Protobuf for FilteredSequencerBlock {
    type Error = FilteredSequencerBlockError;
    type Raw = raw::FilteredSequencerBlock;

    fn try_from_raw_ref(raw: &Self::Raw) -> Result<Self, Self::Error> {
        Self::try_from_raw(raw.clone())
    }

    fn to_raw(&self) -> Self::Raw {
        self.clone().into_raw()
    }

    fn try_from_raw(raw: Self::Raw) -> Result<Self, Self::Error> {
        Self::try_from_raw(raw)
    }

    fn into_raw(self) -> Self::Raw {
        self.into_raw()
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct FilteredSequencerBlockError(FilteredSequencerBlockErrorKind);

#[derive(Debug, thiserror::Error)]
enum FilteredSequencerBlockErrorKind {
    #[error(
        "the block hash in the raw protobuf filtered sequencer block was expected to be 32 bytes \
         long, but was actually `{0}`"
    )]
    InvalidBlockHash(usize),
    #[error("failed to create a sequencer block header from the raw protobuf header")]
    InvalidHeader(SequencerBlockHeaderError),
    #[error("the rollup ID in the raw protobuf rollup transaction was not 32 bytes long")]
    InvalidRollupId(IncorrectRollupIdLength),
    #[error("the expected field in the raw source type was not set: `{0}`")]
    FieldNotSet(&'static str),
    #[error("failed parsing a raw protobuf rollup transaction")]
    ParseRollupTransactions(RollupTransactionsError),
    #[error(
        "the rollup transactions in the sequencer block were not included in the block's data hash"
    )]
    RollupTransactionsNotInSequencerBlock,
    #[error(
        "the rollup transaction for rollup ID `{id}` contained in the filtered sequencer block \
         could not be verified against the rollup transactions root"
    )]
    RollupTransactionForIdNotInSequencerBlock { id: RollupId },
    #[error("the rollup IDs in the sequencer block were not included in the block's data hash")]
    RollupIdsNotInSequencerBlock,
    #[error("failed constructing a transaction proof from the raw protobuf transaction proof")]
    TransactionProofInvalid(merkle::audit::InvalidProof),
    #[error("failed constructing a rollup ID proof from the raw protobuf rollup ID proof")]
    IdProofInvalid(merkle::audit::InvalidProof),
}

impl FilteredSequencerBlockError {
    fn invalid_block_hash(len: usize) -> Self {
        Self(FilteredSequencerBlockErrorKind::InvalidBlockHash(len))
    }

    fn invalid_header(source: SequencerBlockHeaderError) -> Self {
        Self(FilteredSequencerBlockErrorKind::InvalidHeader(source))
    }

    fn invalid_rollup_id(source: IncorrectRollupIdLength) -> Self {
        Self(FilteredSequencerBlockErrorKind::InvalidRollupId(source))
    }

    fn field_not_set(field: &'static str) -> Self {
        Self(FilteredSequencerBlockErrorKind::FieldNotSet(field))
    }

    fn parse_rollup_transactions(source: RollupTransactionsError) -> Self {
        Self(FilteredSequencerBlockErrorKind::ParseRollupTransactions(
            source,
        ))
    }

    fn rollup_transactions_not_in_sequencer_block() -> Self {
        Self(FilteredSequencerBlockErrorKind::RollupTransactionsNotInSequencerBlock)
    }

    fn rollup_transaction_for_id_not_in_sequencer_block(id: RollupId) -> Self {
        Self(
            FilteredSequencerBlockErrorKind::RollupTransactionForIdNotInSequencerBlock {
                id,
            },
        )
    }

    fn rollup_ids_not_in_sequencer_block() -> Self {
        Self(FilteredSequencerBlockErrorKind::RollupIdsNotInSequencerBlock)
    }

    fn transaction_proof_invalid(source: merkle::audit::InvalidProof) -> Self {
        Self(FilteredSequencerBlockErrorKind::TransactionProofInvalid(
            source,
        ))
    }

    fn id_proof_invalid(source: merkle::audit::InvalidProof) -> Self {
        Self(FilteredSequencerBlockErrorKind::IdProofInvalid(source))
    }
}

/// [`Deposit`] represents a deposit from the sequencer to a rollup.
///
/// A [`Deposit`] is constructed whenever a [`BridgeLockAction`] is executed
/// and stored as part of the block's events.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(into = "crate::generated::astria::sequencerblock::v1::Deposit")
)]
pub struct Deposit {
    // the address on the sequencer to which the funds were sent to.
    pub bridge_address: Address,
    // the rollup ID registered to the `bridge_address`
    pub rollup_id: RollupId,
    // the amount that was transferred to `bridge_address`
    pub amount: u128,
    // the IBC ICS20 denom of the asset that was transferred
    pub asset: asset::Denom,
    // the address on the destination chain (rollup) which to send the bridged funds to
    pub destination_chain_address: String,
    // the transaction ID of the source action for the deposit, consisting
    // of the transaction hash.
    pub source_transaction_id: TransactionId,
    // index of the deposit's source action within its transaction
    pub source_action_index: u64,
}

impl Deposit {
    #[must_use]
    pub fn into_raw(self) -> raw::Deposit {
        let Self {
            bridge_address,
            rollup_id,
            amount,
            asset,
            destination_chain_address,
            source_transaction_id,
            source_action_index,
        } = self;
        raw::Deposit {
            bridge_address: Some(bridge_address.into_raw()),
            rollup_id: Some(rollup_id.into_raw()),
            amount: Some(amount.into()),
            asset: asset.to_string(),
            destination_chain_address,
            source_transaction_id: Some(source_transaction_id.into_raw()),
            source_action_index,
        }
    }

    /// Attempts to transform the deposit from its raw representation.
    ///
    /// # Errors
    ///
    /// - if the bridge address is invalid
    /// - if the amount is unset
    /// - if the rollup ID is invalid
    /// - if the asset ID is invalid
    pub fn try_from_raw(raw: raw::Deposit) -> Result<Self, DepositError> {
        let raw::Deposit {
            bridge_address,
            rollup_id,
            amount,
            asset,
            destination_chain_address,
            source_transaction_id,
            source_action_index,
        } = raw;
        let Some(bridge_address) = bridge_address else {
            return Err(DepositError::field_not_set("bridge_address"));
        };
        let bridge_address =
            Address::try_from_raw(bridge_address).map_err(DepositError::address)?;
        let amount = amount.ok_or(DepositError::field_not_set("amount"))?.into();
        let Some(rollup_id) = rollup_id else {
            return Err(DepositError::field_not_set("rollup_id"));
        };
        let rollup_id =
            RollupId::try_from_raw(rollup_id).map_err(DepositError::incorrect_rollup_id_length)?;
        let asset = asset.parse().map_err(DepositError::incorrect_asset)?;
        let Some(source_transaction_id) = source_transaction_id else {
            return Err(DepositError::field_not_set("transaction_id"));
        };
        let source_transaction_id = TransactionId::try_from_raw_ref(&source_transaction_id)
            .map_err(DepositError::transaction_id_error)?;
        Ok(Self {
            bridge_address,
            rollup_id,
            amount,
            asset,
            destination_chain_address,
            source_transaction_id,
            source_action_index,
        })
    }
}

impl From<Deposit> for raw::Deposit {
    fn from(deposit: Deposit) -> Self {
        deposit.into_raw()
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DepositError(DepositErrorKind);

impl DepositError {
    fn address(source: AddressError) -> Self {
        Self(DepositErrorKind::Address {
            source,
        })
    }

    fn field_not_set(field: &'static str) -> Self {
        Self(DepositErrorKind::FieldNotSet(field))
    }

    fn incorrect_rollup_id_length(source: IncorrectRollupIdLength) -> Self {
        Self(DepositErrorKind::IncorrectRollupIdLength(source))
    }

    fn incorrect_asset(source: asset::ParseDenomError) -> Self {
        Self(DepositErrorKind::IncorrectAsset(source))
    }

    fn transaction_id_error(source: TransactionIdError) -> Self {
        Self(DepositErrorKind::TransactionIdError(source))
    }
}

#[derive(Debug, thiserror::Error)]
enum DepositErrorKind {
    #[error("the address is invalid")]
    Address { source: AddressError },
    #[error("the expected field in the raw source type was not set: `{0}`")]
    FieldNotSet(&'static str),
    #[error("the rollup ID length is not 32 bytes")]
    IncorrectRollupIdLength(#[source] IncorrectRollupIdLength),
    #[error("the `asset` field could not be parsed")]
    IncorrectAsset(#[source] asset::ParseDenomError),
    #[error("field `source_transaction_id` was invalid")]
    TransactionIdError(#[source] TransactionIdError),
}

/// A piece of data that is sent to a rollup execution node.
///
/// The data can be either sequenced data (originating from a [`RollupDataSubmission`]
/// action submitted by a user) or a [`Deposit`] (originating from a [`BridgeLock`] action).
///
/// The rollup node receives this type as opaque, protobuf-encoded bytes from conductor,
/// and must decode it accordingly.
#[derive(Debug, Clone, PartialEq)]
pub enum RollupData {
    SequencedData(Bytes),
    Deposit(Box<Deposit>),
}

impl RollupData {
    #[must_use]
    pub fn into_raw(self) -> raw::RollupData {
        match self {
            Self::SequencedData(data) => raw::RollupData {
                value: Some(raw::rollup_data::Value::SequencedData(data)),
            },
            Self::Deposit(deposit) => raw::RollupData {
                value: Some(raw::rollup_data::Value::Deposit(deposit.into_raw())),
            },
        }
    }

    /// Attempts to transform the `RollupData` from its raw representation.
    ///
    /// # Errors
    ///
    /// - if the `data` field is not set
    /// - if the variant is `Deposit` but a `Deposit` cannot be constructed from the raw proto
    pub fn try_from_raw(raw: raw::RollupData) -> Result<Self, RollupDataError> {
        let raw::RollupData {
            value,
        } = raw;
        match value {
            Some(raw::rollup_data::Value::SequencedData(data)) => Ok(Self::SequencedData(data)),
            Some(raw::rollup_data::Value::Deposit(deposit)) => Deposit::try_from_raw(deposit)
                .map(Box::new)
                .map(Self::Deposit)
                .map_err(RollupDataError::deposit),
            None => Err(RollupDataError::field_not_set("data")),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RollupDataError(RollupDataErrorKind);

impl RollupDataError {
    fn field_not_set(field: &'static str) -> Self {
        Self(RollupDataErrorKind::FieldNotSet(field))
    }

    fn deposit(source: DepositError) -> Self {
        Self(RollupDataErrorKind::Deposit(source))
    }
}

#[derive(Debug, thiserror::Error)]
enum RollupDataErrorKind {
    #[error("the expected field in the raw source type was not set: `{0}`")]
    FieldNotSet(&'static str),
    #[error("failed to validate `deposit` field")]
    Deposit(#[source] DepositError),
}
