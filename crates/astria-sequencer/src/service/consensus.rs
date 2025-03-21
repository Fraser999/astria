use astria_core::protocol::genesis::v1::GenesisAppState;
use astria_eyre::eyre::{
    bail,
    Result,
    WrapErr as _,
};
use cnidarium::Storage;
use tendermint::v0_38::abci::{
    request,
    response,
    ConsensusRequest,
    ConsensusResponse,
};
use tokio::sync::mpsc;
use tower_abci::BoxError;
use tower_actor::Message;
use tracing::{
    debug,
    instrument,
    warn,
    Instrument,
    Level,
};

use crate::app::App;

pub(crate) struct Consensus {
    queue: mpsc::Receiver<Message<ConsensusRequest, ConsensusResponse, tower::BoxError>>,
    storage: Storage,
    app: App,
}

impl Consensus {
    pub(crate) fn new(
        storage: Storage,
        app: App,
        queue: mpsc::Receiver<Message<ConsensusRequest, ConsensusResponse, tower::BoxError>>,
    ) -> Self {
        Self {
            queue,
            storage,
            app,
        }
    }

    pub(crate) async fn run(mut self) -> Result<(), tower::BoxError> {
        while let Some(Message {
            req,
            rsp_sender,
            span,
        }) = self.queue.recv().await
        {
            // The send only fails if the receiver was dropped, which happens
            // if the caller didn't propagate the message back to tendermint
            // for some reason -- but that's not our problem.
            let rsp = self.handle_request(req).instrument(span.clone()).await;
            if let Err(e) = rsp.as_ref() {
                panic!("failed to handle consensus request, this is a bug: {e:?}");
            }
            // `send` returns the sent message if sending fail, so we are dropping it.
            if rsp_sender.send(rsp).is_err() {
                warn!(
                    parent: &span,
                    "failed returning consensus response to request sender; dropping response"
                );
            }
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_request(
        &mut self,
        req: ConsensusRequest,
    ) -> Result<ConsensusResponse, BoxError> {
        Ok(match req {
            ConsensusRequest::InitChain(init_chain) => ConsensusResponse::InitChain(
                self.init_chain(init_chain)
                    .await
                    .wrap_err("failed initializing chain")?,
            ),
            ConsensusRequest::PrepareProposal(prepare_proposal) => {
                ConsensusResponse::PrepareProposal(
                    self.handle_prepare_proposal(prepare_proposal)
                        .await
                        .wrap_err("failed to prepare proposal")?,
                )
            }
            ConsensusRequest::ProcessProposal(process_proposal) => {
                ConsensusResponse::ProcessProposal(
                    match self.handle_process_proposal(process_proposal).await {
                        Ok(()) => response::ProcessProposal::Accept,
                        Err(e) => {
                            warn!(
                                error = AsRef::<dyn std::error::Error>::as_ref(&e),
                                "rejecting proposal"
                            );
                            response::ProcessProposal::Reject
                        }
                    },
                )
            }
            ConsensusRequest::ExtendVote(_) => {
                ConsensusResponse::ExtendVote(response::ExtendVote {
                    vote_extension: vec![].into(),
                })
            }
            ConsensusRequest::VerifyVoteExtension(_) => {
                ConsensusResponse::VerifyVoteExtension(response::VerifyVoteExtension::Accept)
            }
            ConsensusRequest::FinalizeBlock(finalize_block) => ConsensusResponse::FinalizeBlock(
                self.finalize_block(finalize_block)
                    .await
                    .wrap_err("failed to finalize block")?,
            ),
            ConsensusRequest::Commit => {
                ConsensusResponse::Commit(self.commit().await.wrap_err("failed to commit")?)
            }
        })
    }

    #[instrument(skip_all, err)]
    async fn init_chain(&mut self, init_chain: request::InitChain) -> Result<response::InitChain> {
        // the storage version is set to u64::MAX by default when first created
        if self.storage.latest_version() != u64::MAX {
            bail!("database already initialized");
        }

        let genesis_state: GenesisAppState = serde_json::from_slice(&init_chain.app_state_bytes)
            .wrap_err("failed to parse app_state in genesis file")?;
        let app_hash = self
            .app
            .init_chain(
                self.storage.clone(),
                genesis_state,
                init_chain
                    .validators
                    .iter()
                    .cloned()
                    .map(crate::utils::cometbft_to_sequencer_validator)
                    .collect::<Result<_, _>>()
                    .wrap_err(
                        "failed converting cometbft genesis validators to astria validators",
                    )?,
                init_chain.chain_id,
            )
            .await
            .wrap_err("failed to call init_chain")?;
        self.app.commit(self.storage.clone()).await;

        Ok(response::InitChain {
            app_hash,
            consensus_params: Some(init_chain.consensus_params),
            validators: init_chain.validators,
        })
    }

    #[instrument(skip_all, err(level = Level::WARN))]
    async fn handle_prepare_proposal(
        &mut self,
        prepare_proposal: request::PrepareProposal,
    ) -> Result<response::PrepareProposal> {
        self.app
            .prepare_proposal(prepare_proposal, self.storage.clone())
            .await
    }

    #[instrument(skip_all, err(level = Level::WARN))]
    async fn handle_process_proposal(
        &mut self,
        process_proposal: request::ProcessProposal,
    ) -> Result<()> {
        self.app
            .process_proposal(process_proposal, self.storage.clone())
            .await?;
        debug!("proposal processed");
        Ok(())
    }

    #[instrument(skip_all, err)]
    async fn finalize_block(
        &mut self,
        finalize_block: request::FinalizeBlock,
    ) -> Result<response::FinalizeBlock> {
        let finalize_block = self
            .app
            .finalize_block(finalize_block, self.storage.clone())
            .await
            .wrap_err("failed to call App::finalize_block")?;
        Ok(finalize_block)
    }

    #[instrument(skip_all)]
    async fn commit(&mut self) -> Result<response::Commit> {
        self.app.commit(self.storage.clone()).await;
        Ok(response::Commit::default())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        str::FromStr,
        sync::Arc,
    };

    use astria_core::{
        crypto::{
            SigningKey,
            VerificationKey,
        },
        primitive::v1::RollupId,
        protocol::transaction::v1::{
            action::RollupDataSubmission,
            TransactionBody,
        },
        Protobuf as _,
    };
    use bytes::Bytes;
    use prost::Message as _;
    use rand::rngs::OsRng;
    use telemetry::Metrics as _;
    use tendermint::{
        account::Id,
        Hash,
        Time,
    };

    use super::*;
    use crate::{
        app::benchmark_and_test_utils::{
            mock_balances,
            mock_tx_cost,
        },
        mempool::Mempool,
        metrics::Metrics,
        proposal::commitment::generate_rollup_datas_commitment,
    };

    fn make_unsigned_tx() -> TransactionBody {
        TransactionBody::builder()
            .actions(vec![RollupDataSubmission {
                rollup_id: RollupId::from_unhashed_bytes(b"testchainid"),
                data: Bytes::from_static(b"hello world"),
                fee_asset: crate::benchmark_and_test_utils::nria().into(),
            }
            .into()])
            .chain_id("test")
            .try_build()
            .unwrap()
    }

    fn new_prepare_proposal_request() -> request::PrepareProposal {
        request::PrepareProposal {
            txs: vec![],
            max_tx_bytes: 1024,
            local_last_commit: None,
            misbehavior: vec![],
            height: 1u32.into(),
            time: Time::now(),
            next_validators_hash: Hash::default(),
            proposer_address: Id::from_str("0CDA3F47EF3C4906693B170EF650EB968C5F4B2C").unwrap(),
        }
    }

    fn new_process_proposal_request(txs: Vec<Bytes>) -> request::ProcessProposal {
        request::ProcessProposal {
            txs,
            proposed_last_commit: None,
            misbehavior: vec![],
            hash: Hash::try_from([0u8; 32].to_vec()).unwrap(),
            height: 1u32.into(),
            next_validators_hash: Hash::default(),
            time: Time::now(),
            proposer_address: Id::from_str("0CDA3F47EF3C4906693B170EF650EB968C5F4B2C").unwrap(),
        }
    }

    #[tokio::test]
    async fn prepare_and_process_proposal() {
        let signing_key = SigningKey::new(OsRng);
        let (mut consensus_service, mempool) =
            new_consensus_service(Some(signing_key.verification_key())).await;
        let tx = make_unsigned_tx();
        let signed_tx = Arc::new(tx.sign(&signing_key));
        let tx_bytes = signed_tx.to_raw().encode_to_vec();
        let txs = vec![tx_bytes.into()];
        mempool
            .insert(
                signed_tx.clone(),
                0,
                mock_balances(0, 0),
                mock_tx_cost(0, 0, 0),
            )
            .await
            .unwrap();

        let res = generate_rollup_datas_commitment(&vec![(*signed_tx).clone()], HashMap::new());

        let prepare_proposal = new_prepare_proposal_request();
        let prepare_proposal_response = consensus_service
            .handle_prepare_proposal(prepare_proposal)
            .await
            .unwrap();
        assert_eq!(
            prepare_proposal_response,
            response::PrepareProposal {
                txs: res.into_transactions(txs)
            }
        );

        let (mut consensus_service, _) =
            new_consensus_service(Some(signing_key.verification_key())).await;
        let process_proposal = new_process_proposal_request(prepare_proposal_response.txs);
        consensus_service
            .handle_process_proposal(process_proposal)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn process_proposal_ok() {
        let signing_key = SigningKey::new(OsRng);
        let (mut consensus_service, _) =
            new_consensus_service(Some(signing_key.verification_key())).await;
        let tx = make_unsigned_tx();
        let signed_tx = tx.sign(&signing_key);
        let tx_bytes = signed_tx.clone().into_raw().encode_to_vec();
        let txs = vec![tx_bytes.into()];
        let res = generate_rollup_datas_commitment(&vec![signed_tx], HashMap::new());
        let process_proposal = new_process_proposal_request(res.into_transactions(txs));
        consensus_service
            .handle_process_proposal(process_proposal)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn process_proposal_fail_missing_action_commitment() {
        let (mut consensus_service, _) = new_consensus_service(None).await;
        let process_proposal = new_process_proposal_request(vec![]);
        assert!(consensus_service
            .handle_process_proposal(process_proposal)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("no transaction commitment in proposal"));
    }

    #[tokio::test]
    async fn process_proposal_fail_wrong_commitment_length() {
        let (mut consensus_service, _) = new_consensus_service(None).await;
        let process_proposal = new_process_proposal_request(vec![[0u8; 16].to_vec().into()]);
        assert!(consensus_service
            .handle_process_proposal(process_proposal)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("transaction commitment must be 32 bytes"));
    }

    #[tokio::test]
    async fn process_proposal_fail_wrong_commitment_value() {
        let (mut consensus_service, _) = new_consensus_service(None).await;
        let process_proposal = new_process_proposal_request(vec![
            [99u8; 32].to_vec().into(),
            [99u8; 32].to_vec().into(),
        ]);
        assert!(consensus_service
            .handle_process_proposal(process_proposal)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("transaction commitment does not match expected"));
    }

    #[tokio::test]
    async fn prepare_proposal_empty_block() {
        let (mut consensus_service, _) = new_consensus_service(None).await;
        let txs = vec![];
        let res = generate_rollup_datas_commitment(&txs.clone(), HashMap::new());
        let prepare_proposal = new_prepare_proposal_request();

        let prepare_proposal_response = consensus_service
            .handle_prepare_proposal(prepare_proposal)
            .await
            .unwrap();
        assert_eq!(
            prepare_proposal_response,
            response::PrepareProposal {
                txs: res.into_transactions(vec![]),
            }
        );
    }

    #[tokio::test]
    async fn process_proposal_ok_empty_block() {
        let (mut consensus_service, _) = new_consensus_service(None).await;
        let txs = vec![];
        let res = generate_rollup_datas_commitment(&txs, HashMap::new());
        let process_proposal = new_process_proposal_request(res.into_transactions(vec![]));
        consensus_service
            .handle_process_proposal(process_proposal)
            .await
            .unwrap();
    }

    /// Returns a default tendermint block header for test purposes.
    fn default_header() -> tendermint::block::Header {
        use tendermint::{
            account,
            block::{
                header::Version,
                Height,
            },
            chain,
            hash::AppHash,
        };

        tendermint::block::Header {
            version: Version {
                block: 0,
                app: 0,
            },
            chain_id: chain::Id::try_from("test").unwrap(),
            height: Height::from(1u32),
            time: Time::now(),
            last_block_id: None,
            last_commit_hash: None,
            data_hash: None,
            validators_hash: Hash::Sha256([0; 32]),
            next_validators_hash: Hash::Sha256([0; 32]),
            consensus_hash: Hash::Sha256([0; 32]),
            app_hash: AppHash::try_from([0; 32].to_vec()).unwrap(),
            last_results_hash: None,
            evidence_hash: None,
            proposer_address: account::Id::try_from([0u8; 20].to_vec()).unwrap(),
        }
    }

    async fn new_consensus_service(funded_key: Option<VerificationKey>) -> (Consensus, Mempool) {
        let accounts = if let Some(funded_key) = funded_key {
            vec![
                astria_core::generated::astria::protocol::genesis::v1::Account {
                    address: Some(
                        crate::benchmark_and_test_utils::astria_address(funded_key.address_bytes())
                            .to_raw(),
                    ),
                    balance: Some(10u128.pow(19).into()),
                },
            ]
        } else {
            vec![]
        };
        let genesis_state = {
            let mut state = crate::app::benchmark_and_test_utils::proto_genesis_state();
            state.accounts = accounts;
            state
        }
        .try_into()
        .unwrap();

        let storage = cnidarium::TempStorage::new().await.unwrap();
        let snapshot = storage.latest_snapshot();
        let metrics = Box::leak(Box::new(Metrics::noop_metrics(&()).unwrap()));
        let mempool = Mempool::new(metrics, 100);
        let mut app = App::new(snapshot, mempool.clone(), metrics).await.unwrap();
        app.init_chain(storage.clone(), genesis_state, vec![], "test".to_string())
            .await
            .unwrap();
        app.commit(storage.clone()).await;

        let (_tx, rx) = mpsc::channel(1);
        (Consensus::new(storage.clone(), app, rx), mempool)
    }

    #[tokio::test]
    async fn block_lifecycle() {
        use sha2::Digest as _;

        let signing_key = SigningKey::new(OsRng);
        let address_bytes = *signing_key.verification_key().address_bytes();
        let (mut consensus_service, mempool) =
            new_consensus_service(Some(signing_key.verification_key())).await;

        let tx = make_unsigned_tx();
        let signed_tx = Arc::new(tx.sign(&signing_key));
        let tx_bytes = signed_tx.to_raw().encode_to_vec();
        let txs = vec![tx_bytes.clone().into()];
        let res = generate_rollup_datas_commitment(&vec![(*signed_tx).clone()], HashMap::new());

        let block_data = res.into_transactions(txs.clone());
        let data_hash =
            merkle::Tree::from_leaves(block_data.iter().map(sha2::Sha256::digest)).root();
        let mut header = default_header();
        header.data_hash = Some(Hash::try_from(data_hash.to_vec()).unwrap());

        mempool
            .insert(signed_tx, 0, mock_balances(0, 0), mock_tx_cost(0, 0, 0))
            .await
            .unwrap();

        let process_proposal = new_process_proposal_request(block_data.clone());
        consensus_service
            .handle_request(ConsensusRequest::ProcessProposal(process_proposal))
            .await
            .unwrap();

        let finalize_block = request::FinalizeBlock {
            hash: Hash::try_from([0u8; 32].to_vec()).unwrap(),
            height: 1u32.into(),
            time: Time::now(),
            next_validators_hash: Hash::default(),
            proposer_address: [0u8; 20].to_vec().try_into().unwrap(),
            decided_last_commit: tendermint::abci::types::CommitInfo {
                round: 0u16.into(),
                votes: vec![],
            },
            misbehavior: vec![],
            txs: block_data,
        };
        consensus_service
            .handle_request(ConsensusRequest::FinalizeBlock(finalize_block))
            .await
            .unwrap();

        // Mempool should still have a transaction
        assert_eq!(mempool.len().await, 1);
        assert_eq!(mempool.pending_nonce(&address_bytes).await, Some(1));

        let commit = ConsensusRequest::Commit {};
        consensus_service.handle_request(commit).await.unwrap();

        // ensure that txs included in a block are removed from the mempool
        assert_eq!(mempool.len().await, 0);
        assert_eq!(mempool.pending_nonce(&address_bytes).await, None);
    }
}
