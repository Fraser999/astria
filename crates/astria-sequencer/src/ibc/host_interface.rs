use cnidarium::StateRead;
use penumbra_ibc::component::HostInterface;

// use crate::state_ext::StateReadExt as _;

#[derive(Clone)]
pub(crate) struct AstriaHost;

#[async_trait::async_trait]
impl HostInterface for AstriaHost {
    async fn get_chain_id<S: StateRead>(_state: S) -> anyhow::Result<String> {
        unreachable!();
        // state.get_chain_id().await.map(|s| s.to_string())
    }

    async fn get_revision_number<S: StateRead>(_state: S) -> anyhow::Result<u64> {
        unreachable!();
        // state.get_revision_number().await
    }

    async fn get_block_height<S: StateRead>(_state: S) -> anyhow::Result<u64> {
        unreachable!();
        // state.get_block_height().await
    }

    async fn get_block_timestamp<S: StateRead>(_state: S) -> anyhow::Result<tendermint::Time> {
        unreachable!();
        // state.get_block_timestamp().await
    }
}
