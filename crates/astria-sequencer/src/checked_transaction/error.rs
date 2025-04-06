use astria_core::{
    generated::astria::protocol::transaction::v1 as raw,
    protocol::transaction::v1::TransactionError,
};
use astria_eyre::eyre;
use prost::{
    DecodeError,
    Name as _,
};
use thiserror::Error;

use super::MAX_TX_BYTES;
use crate::checked_actions::CheckedActionError;

#[derive(Debug, Error)]
pub(crate) enum CheckedTransactionError {
    #[error("transaction size too large; allowed {MAX_TX_BYTES} bytes, got {tx_len} bytes")]
    TooLarge { tx_len: usize },

    #[error(
        "failed decoding bytes as a protobuf `{}`: {0:#}",
        raw::Transaction::full_name()
    )]
    Decode(#[source] DecodeError),

    #[error(
        "failed converting protobuf `{}` to domain transaction: {0:#}",
        raw::Transaction::full_name()
    )]
    Convert(#[source] TransactionError),

    #[error("must have at least one action")]
    NoActions,

    #[error(
        "transaction for wrong chain; expected chain id `{expected}`, transaction chain id \
         `{tx_chain_id}`"
    )]
    ChainIdMismatch {
        expected: String,
        tx_chain_id: String,
    },

    #[error(
        "invalid transaction nonce; expected nonce `{expected}`, transaction nonce `{tx_nonce}`"
    )]
    InvalidNonce { expected: u32, tx_nonce: u32 },

    #[error("overflow occurred incrementing stored nonce")]
    NonceOverflowed,

    #[error("overflow occurred incrementing action index")]
    ActionIndexOverflowed,

    #[error(transparent)]
    CheckedAction(#[from] CheckedActionError),

    #[error("internal error: {context}: {source:#}")]
    InternalError {
        context: String,
        source: eyre::Report,
    },
}

impl CheckedTransactionError {
    pub(super) fn internal(context: &str, source: eyre::Report) -> Self {
        Self::InternalError {
            context: context.to_string(),
            source,
        }
    }
}
