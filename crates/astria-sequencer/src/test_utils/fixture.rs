use std::time::Duration;

use astria_core::{
    crypto::ADDRESS_LENGTH,
    primitive::v1::{
        asset::IbcPrefixed,
        Address,
        TransactionId,
    },
    protocol::{
        genesis::v1::GenesisAppState,
        transaction::v1::Action,
    },
};
use cnidarium::{
    Snapshot,
    StateDelta,
    TempStorage,
};
use futures::TryStreamExt as _;
use ibc_types::{
    core::{
        client::ClientId,
        commitment::MerkleRoot,
    },
    lightclients::tendermint::{
        client_state::ClientState,
        ConsensusState,
    },
};
use penumbra_ibc::component::{
    ClientStateWriteExt as _,
    ConsensusStateWriteExt as _,
};
use telemetry::Metrics as _;
use tendermint::abci;

use super::{
    BridgeInitializer,
    ChainInitializer,
};
use crate::{
    accounts::{
        AddressBytes,
        StateReadExt as _,
    },
    app::{
        App,
        StateReadExt as _,
        StateWriteExt as _,
    },
    benchmark_and_test_utils::nria,
    checked_actions::{
        CheckedAction,
        CheckedActionError,
    },
    fees::StateReadExt as _,
    ibc::host_interface::AstriaHost,
    mempool::Mempool,
    Metrics,
};

pub(crate) struct Fixture {
    pub(crate) app: App,
    pub(super) storage: TempStorage,
    pub(super) genesis_app_state: Option<GenesisAppState>,
}

impl Fixture {
    pub(crate) async fn uninitialized() -> Self {
        let storage = TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let metrics = Box::leak(Box::new(Metrics::noop_metrics(&()).unwrap()));
        let mempool = Mempool::new(metrics, 100);
        let app = App::new(snapshot, mempool, metrics).await.unwrap();
        Self {
            storage,
            app,
            genesis_app_state: None,
        }
    }

    pub(crate) async fn default_initialized() -> Self {
        let mut fixture = Self::uninitialized().await;
        fixture.chain_initializer().init().await;
        fixture
    }

    pub(crate) fn state(&self) -> &StateDelta<Snapshot> {
        self.app.state()
    }

    pub(crate) fn state_mut(&mut self) -> &mut StateDelta<Snapshot> {
        self.app.state_mut()
    }

    pub(crate) fn metrics(&self) -> &'static Metrics {
        self.app.metrics()
    }

    pub(crate) fn into_events(self) -> Vec<abci::Event> {
        self.app.into_events()
    }

    pub(crate) fn chain_id(&self) -> &str {
        self.genesis_app_state().chain_id()
    }

    pub(crate) fn chain_initializer(&mut self) -> ChainInitializer<'_> {
        ChainInitializer::new(self)
    }

    pub(crate) fn bridge_initializer(&mut self, bridge_address: Address) -> BridgeInitializer<'_> {
        BridgeInitializer::new(self, bridge_address)
    }

    pub(crate) async fn new_checked_action<T: Into<Action>>(
        &self,
        action: T,
        tx_signer: [u8; ADDRESS_LENGTH],
    ) -> Result<CheckedAction, CheckedActionError> {
        match action.into() {
            Action::RollupDataSubmission(action) => {
                CheckedAction::new_rollup_data_submission(action)
            }
            Action::Transfer(action) => {
                CheckedAction::new_transfer(action, tx_signer, self.state()).await
            }
            Action::ValidatorUpdate(action) => {
                CheckedAction::new_validator_update(action, tx_signer, self.state()).await
            }
            Action::SudoAddressChange(action) => {
                CheckedAction::new_sudo_address_change(action, tx_signer, self.state()).await
            }
            Action::Ibc(action) => {
                CheckedAction::new_ibc_relay(action, tx_signer, self.state()).await
            }
            Action::IbcSudoChange(action) => {
                CheckedAction::new_ibc_sudo_change(action, tx_signer, self.state()).await
            }
            Action::Ics20Withdrawal(action) => {
                CheckedAction::new_ics20_withdrawal(action, tx_signer, self.state()).await
            }
            Action::IbcRelayerChange(action) => {
                CheckedAction::new_ibc_relayer_change(action, tx_signer, self.state()).await
            }
            Action::FeeAssetChange(action) => {
                CheckedAction::new_fee_asset_change(action, tx_signer, self.state()).await
            }
            Action::InitBridgeAccount(action) => {
                CheckedAction::new_init_bridge_account(action, tx_signer, self.state()).await
            }
            Action::BridgeLock(action) => {
                CheckedAction::new_bridge_lock(
                    action,
                    tx_signer,
                    TransactionId::new([10; 32]),
                    10,
                    self.state(),
                )
                .await
            }
            Action::BridgeUnlock(action) => {
                CheckedAction::new_bridge_unlock(action, tx_signer, self.state()).await
            }
            Action::BridgeSudoChange(action) => {
                CheckedAction::new_bridge_sudo_change(action, tx_signer, self.state()).await
            }
            Action::BridgeTransfer(action) => {
                CheckedAction::new_bridge_transfer(
                    action,
                    tx_signer,
                    TransactionId::new([11; 32]),
                    11,
                    self.state(),
                )
                .await
            }
            Action::FeeChange(action) => {
                CheckedAction::new_fee_change(action, tx_signer, self.state()).await
            }
            Action::RecoverIbcClient(action) => {
                CheckedAction::new_recover_ibc_client(action, tx_signer, self.state()).await
            }
        }
    }

    pub(crate) async fn allowed_fee_assets(&self) -> Vec<IbcPrefixed> {
        self.state()
            .allowed_fee_assets()
            .try_collect()
            .await
            .unwrap()
    }

    pub(crate) async fn get_nria_balance<TAddress: AddressBytes>(
        &self,
        address: &TAddress,
    ) -> u128 {
        self.state()
            .get_account_balance(address, &nria())
            .await
            .unwrap()
    }

    pub(crate) async fn authority_component_end_block(&mut self) {
        self.app.authority_component_end_block().await;
    }

    pub(crate) async fn init_active_ibc_client(
        &mut self,
        client_id: &ClientId,
        client_state: ClientState,
    ) {
        self.init_ibc_client(client_id, client_state, true).await;
    }

    pub(crate) async fn init_expired_ibc_client(
        &mut self,
        client_id: &ClientId,
        client_state: ClientState,
    ) {
        self.init_ibc_client(client_id, client_state, false).await;
    }

    async fn init_ibc_client(
        &mut self,
        client_id: &ClientId,
        client_state: ClientState,
        active: bool,
    ) {
        let height = client_state.latest_height;
        let trusting_period = client_state.trusting_period;
        self.state_mut().put_client(client_id, client_state);

        self.state_mut()
            .put_revision_number(height.revision_number)
            .unwrap();
        // Don't allow the stored block height to decrease.
        let current_stored_height = self.state().get_block_height().await.unwrap_or_default();
        self.state_mut()
            .put_block_height(std::cmp::max(height.revision_height, current_stored_height))
            .unwrap();

        let timestamp = tendermint::Time::from_unix_timestamp(100, 2).unwrap();
        self.state_mut().put_block_timestamp(timestamp).unwrap();

        let consensus_state_timestamp = if active {
            // If we want the client to be active, just use the block timestamp for its consensus
            // state.
            timestamp
        } else {
            // If we want the client to be expired, make its consensus state timestamp earlier than
            // the block timestamp by more than the trusting period.
            timestamp
                .checked_sub(trusting_period)
                .and_then(|t| t.checked_sub(Duration::from_nanos(1)))
                .unwrap()
        };
        let consensus_state = ConsensusState::new(
            MerkleRoot {
                hash: vec![1; 32],
            },
            consensus_state_timestamp,
            tendermint::Hash::Sha256([2; 32]),
        );

        self.state_mut()
            .put_verified_consensus_state::<AstriaHost>(height, client_id.clone(), consensus_state)
            .await
            .unwrap();
    }

    fn genesis_app_state(&self) -> &GenesisAppState {
        self.genesis_app_state
            .as_ref()
            .expect("fixture should be initialized")
    }
}
