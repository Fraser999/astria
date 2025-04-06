use astria_core::{
    crypto::ADDRESS_LENGTH,
    primitive::v1::asset::Denom,
};
use astria_eyre::eyre;
use thiserror::Error;

use crate::accounts::AddressBytes;

#[derive(Debug, Error)]
pub(crate) enum CheckedActionError {
    #[error("`{action_name}` action failed initial check")]
    InitialCheck {
        action_name: &'static str,
        source: eyre::Report,
    },

    #[error("`{action_name}` action failed mutable check")]
    MutableCheck {
        action_name: &'static str,
        source: eyre::Report,
    },

    #[error("`{action_name}` action failed execution")]
    Execution {
        action_name: &'static str,
        source: eyre::Report,
    },

    #[error("`{action_name}` action is disabled")]
    ActionDisabled { action_name: &'static str },

    #[error("fee asset {fee_asset} for `{action_name}` action is not allowed")]
    FeeAssetIsNotAllowed {
        fee_asset: Denom,
        action_name: &'static str,
    },

    #[error(
        "insufficient {asset} balance in {} account to pay fee of {amount}",
        account.display_address()
    )]
    InsufficientBalanceToPayFee {
        account: [u8; ADDRESS_LENGTH],
        asset: Denom,
        amount: u128,
    },

    #[error("internal error: {context}")]
    InternalError {
        context: String,
        source: eyre::Report,
    },
}

impl CheckedActionError {
    pub(super) fn initial_check(action_name: &'static str, source: eyre::Report) -> Self {
        Self::InitialCheck {
            action_name,
            source,
        }
    }

    pub(super) fn execution(action_name: &'static str, source: eyre::Report) -> Self {
        Self::Execution {
            action_name,
            source,
        }
    }

    pub(super) fn internal(context: &str, source: eyre::Report) -> Self {
        Self::InternalError {
            context: context.to_string(),
            source,
        }
    }
}
