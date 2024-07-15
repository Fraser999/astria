#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
#[repr(u32)]
pub enum AbciErrorCode {
    UnknownPath = 1,
    InvalidParameter = 2,
    InternalError = 3,
    InvalidNonce = 4,
    TransactionTooLarge = 5,
    InsufficientFunds = 6,
    InvalidChainId = 7,
    ValueNotFound = 8,
    TransactionExpired = 9,
    TransactionFailed = 10,
    BadRequest = 11,
}

impl AbciErrorCode {
    #[must_use]
    pub const fn into_tendermint_code(self) -> tendermint::abci::Code {
        unsafe { tendermint::abci::Code::Err(std::num::NonZeroU32::new_unchecked(self as u32)) }
    }
}

impl std::fmt::Display for AbciErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AbciErrorCode::UnknownPath => "provided path is unknown",
            AbciErrorCode::InvalidParameter => "one or more path parameters were invalid",
            AbciErrorCode::InternalError => "an internal server error occurred",
            AbciErrorCode::InvalidNonce => "the provided nonce was invalid",
            AbciErrorCode::TransactionTooLarge => "the provided transaction was too large",
            AbciErrorCode::InsufficientFunds => "insufficient funds",
            AbciErrorCode::InvalidChainId => "the provided chain id was invalid",
            AbciErrorCode::ValueNotFound => "the requested value was not found",
            AbciErrorCode::TransactionExpired => "the transaction expired in the app's mempool",
            AbciErrorCode::TransactionFailed => {
                "the transaction failed to execute in prepare_proposal()"
            }
            AbciErrorCode::BadRequest => "the request payload was malformed",
        })
    }
}
