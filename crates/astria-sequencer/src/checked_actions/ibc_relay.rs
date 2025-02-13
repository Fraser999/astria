use std::fmt::{
    self,
    Debug,
    Formatter,
};

use astria_core::primitive::v1::ADDRESS_LEN;
use astria_eyre::{
    anyhow_to_eyre,
    eyre::{
        ensure,
        Result,
        WrapErr as _,
    },
};
use cnidarium::{
    StateRead,
    StateWrite,
};
use penumbra_ibc::{
    IbcRelay,
    IbcRelayWithHandlers,
};
use tracing::{
    instrument,
    Level,
};

use super::TransactionSignerAddressBytes;
use crate::ibc::{
    host_interface::AstriaHost,
    ics20_transfer::Ics20Transfer,
    StateReadExt as _,
};

pub(crate) struct CheckedIbcRelay {
    action: IbcRelayWithHandlers<Ics20Transfer, AstriaHost>,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedIbcRelay {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: IbcRelay,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        let action = action.clone().with_handler::<Ics20Transfer, AstriaHost>();

        // Run immutable checks.
        action
            .check_stateless(())
            .await
            .map_err(anyhow_to_eyre)
            .wrap_err("stateless checks failed for ibc action")?;

        let checked_action = Self {
            action,
            tx_signer: tx_signer.into(),
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;
        self.action
            .check_and_execute(state)
            .await
            .map_err(anyhow_to_eyre)
            .wrap_err("failed executing ibc action")
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        ensure!(
            state
                .is_ibc_relayer(&self.tx_signer)
                .await
                .wrap_err("failed to check if address is IBC relayer")?,
            "transaction signer not authorized to execute IBC actions"
        );
        Ok(())
    }
}

impl Debug for CheckedIbcRelay {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedIbcRelay")
            .field("action", &self.action.action())
            .field("tx_signer", &self.tx_signer)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use astria_core::protocol::transaction::v1::action::IbcRelayerChange;
    use ibc_proto::google::protobuf::Any;
    use ibc_types::core::client::{
        msgs::MsgCreateClient,
        ClientId,
    };
    use penumbra_ibc::component::ClientStateReadExt as _;

    use super::{
        super::test_utils::{
            new_client_state,
            new_create_client,
            Fixture,
        },
        *,
    };
    use crate::{
        app::StateWriteExt as _,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
        },
        checked_actions::CheckedAction,
        ibc::StateWriteExt as _,
    };

    #[tokio::test]
    async fn should_fail_construction_if_stateless_checks_fail() {
        let fixture = Fixture::new().await;

        let action = IbcRelay::CreateClient(MsgCreateClient {
            client_state: Any::default(),
            consensus_state: Any::default(),
            signer: String::new(),
        });
        let err = CheckedIbcRelay::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "stateless checks failed for ibc action");
    }

    #[tokio::test]
    async fn should_fail_construction_if_signer_not_authorized() {
        let fixture = Fixture::new().await;

        let action = new_create_client();
        let err = CheckedIbcRelay::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to execute IBC actions",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_signer_not_authorized() {
        let mut fixture = Fixture::new().await;
        // Store the tx signer address as the IBC sudo and relayer address.
        fixture
            .state
            .put_ibc_sudo_address(fixture.tx_signer)
            .unwrap();
        fixture
            .state
            .put_ibc_relayer_address(&fixture.tx_signer)
            .unwrap();

        // Construct the checked action while the tx signer is recorded as the IBC relayer.
        let action = new_create_client();
        let checked_action = CheckedIbcRelay::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        // Remove the IBC relayer.
        let remove_relayer_action = IbcRelayerChange::Removal(astria_address(&fixture.tx_signer));
        let checked_remove_relayer_action = CheckedAction::new_ibc_relayer_change(
            remove_relayer_action,
            fixture.tx_signer,
            &fixture.state,
        )
        .await
        .unwrap();
        checked_remove_relayer_action
            .execute(&mut fixture.state)
            .await
            .unwrap();

        // Try to execute the checked action now - should fail due to signer no longer being
        // authorized.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to execute IBC actions",
        );
    }

    #[tokio::test]
    async fn should_execute() {
        let mut fixture = Fixture::new().await;
        fixture.state.put_block_height(1).unwrap();
        fixture.state.put_revision_number(1).unwrap();
        let timestamp = tendermint::Time::from_unix_timestamp(1, 0).unwrap();
        fixture.state.put_block_timestamp(timestamp).unwrap();

        fixture
            .state
            .put_ibc_relayer_address(&fixture.tx_signer)
            .unwrap();

        let action = new_create_client();
        let checked_action = CheckedIbcRelay::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        checked_action.execute(&mut fixture.state).await.unwrap();

        let client_state = fixture
            .state
            .get_client_state(&ClientId::default())
            .await
            .unwrap();
        assert_eq!(client_state, new_client_state().try_into().unwrap());
    }
}
