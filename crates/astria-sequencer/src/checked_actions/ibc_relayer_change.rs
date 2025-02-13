use astria_core::{
    primitive::v1::ADDRESS_LEN,
    protocol::transaction::v1::action::IbcRelayerChange,
};
use astria_eyre::eyre::{
    ensure,
    Result,
    WrapErr as _,
};
use cnidarium::{
    StateRead,
    StateWrite,
};
use tracing::{
    instrument,
    Level,
};

use super::TransactionSignerAddressBytes;
use crate::{
    address::StateReadExt as _,
    ibc::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

#[derive(Debug)]
pub(crate) struct CheckedIbcRelayerChange {
    action: IbcRelayerChange,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedIbcRelayerChange {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: IbcRelayerChange,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        // Immutable checks for base prefix.
        match &action {
            IbcRelayerChange::Addition(address) | IbcRelayerChange::Removal(address) => {
                state
                    .ensure_base_prefix(address)
                    .await
                    .wrap_err("ibc relayer change address has an unsupported prefix")?;
            }
        }

        let checked_action = Self {
            action,
            tx_signer: tx_signer.into(),
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;

        match self.action {
            IbcRelayerChange::Addition(address) => {
                state
                    .put_ibc_relayer_address(&address)
                    .wrap_err("failed to write ibc relayer address to storage")?;
            }
            IbcRelayerChange::Removal(address) => {
                state.delete_ibc_relayer_address(&address);
            }
        }

        Ok(())
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        // Check that the signer of this tx is the authorized IBC sudo address.
        let ibc_sudo_address = state
            .get_ibc_sudo_address()
            .await
            .wrap_err("failed to read ibc sudo address from storage")?;
        ensure!(
            &ibc_sudo_address == self.tx_signer.as_bytes(),
            "transaction signer not authorized to change ibc relayer",
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astria_core::primitive::v1::TransactionId;

    use super::*;
    use crate::{
        accounts::AddressBytes as _,
        address::StateWriteExt as _,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            ASTRIA_PREFIX,
        },
        transaction::{
            StateWriteExt as _,
            TransactionContext,
        },
    };

    #[test]
    fn todo() {
        todo!("write tests");
    }

//     #[tokio::test]
//     async fn ibc_relayer_addition_executes_as_expected() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = cnidarium::StateDelta::new(snapshot);
//
//         let ibc_sudo_address = astria_address(&[1; 20]);
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: *ibc_sudo_address.address_bytes(),
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//         state.put_ibc_sudo_address(ibc_sudo_address).unwrap();
//
//         let address_to_add = astria_address(&[0; 20]);
//         let action = IbcRelayerChange::Addition(address_to_add);
//         action.check_and_execute(&mut state).await.unwrap();
//
//         assert!(state.is_ibc_relayer(address_to_add).await.unwrap());
//     }
//
//     #[tokio::test]
//     async fn ibc_relayer_removal_executes_as_expected() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = cnidarium::StateDelta::new(snapshot);
//
//         let address_to_remove = astria_address(&[0; 20]);
//         let ibc_sudo_address = astria_address(&[1; 20]);
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: *ibc_sudo_address.address_bytes(),
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//         state.put_ibc_sudo_address(ibc_sudo_address).unwrap();
//         state.put_ibc_relayer_address(&address_to_remove).unwrap();
//
//         assert!(state.is_ibc_relayer(address_to_remove).await.unwrap());
//
//         let action = IbcRelayerChange::Removal(address_to_remove);
//         action.check_and_execute(&mut state).await.unwrap();
//
//         assert!(!state.is_ibc_relayer(address_to_remove).await.unwrap());
//     }
//
//     #[tokio::test]
//     async fn ibc_relayer_addition_fails_if_address_is_not_base_prefixed() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = cnidarium::StateDelta::new(snapshot);
//
//         let different_prefix = "different_prefix";
//         state.put_base_prefix(different_prefix.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: [0; 20],
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//
//         let action = IbcRelayerChange::Addition(astria_address(&[0; 20]));
//         assert_eyre_error(
//             &action.check_and_execute(&mut state).await.unwrap_err(),
//             "failed check for base prefix of provided address to be added/removed",
//         );
//     }
//
//     #[tokio::test]
//     async fn ibc_relayer_removal_fails_if_address_is_not_base_prefixed() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = cnidarium::StateDelta::new(snapshot);
//
//         let different_prefix = "different_prefix";
//         state.put_base_prefix(different_prefix.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: [0; 20],
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//
//         let action = IbcRelayerChange::Removal(astria_address(&[0; 20]));
//         assert_eyre_error(
//             &action.check_and_execute(&mut state).await.unwrap_err(),
//             "failed check for base prefix of provided address to be added/removed",
//         );
//     }
//
//     #[tokio::test]
//     async fn ibc_relayer_change_fails_if_signer_is_not_sudo_address() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = cnidarium::StateDelta::new(snapshot);
//
//         let ibc_sudo_address = astria_address(&[1; 20]);
//         let signer = astria_address(&[2; 20]);
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: *signer.address_bytes(),
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//         state.put_ibc_sudo_address(ibc_sudo_address).unwrap();
//
//         let action = IbcRelayerChange::Addition(astria_address(&[0; 20]));
//         assert_eyre_error(
//             &action.check_and_execute(&mut state).await.unwrap_err(),
//             "unauthorized address for IBC relayer change",
//         );
//     }
}
