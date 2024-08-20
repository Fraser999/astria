use anyhow::{
    anyhow,
    Context,
    Result,
};
use async_trait::async_trait;
use tracing::instrument;

use crate::storage::{
    Fee,
    StateRead,
    StateWrite,
};

const SEQUENCE_ACTION_BASE_FEE_STORAGE_KEY: &str = "seqbasefee";
const SEQUENCE_ACTION_BYTE_COST_MULTIPLIER_STORAGE_KEY: &str = "seqmultiplier";

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_sequence_action_base_fee(&self) -> Result<u128> {
        Ok(self
            .get::<_, Fee>(SEQUENCE_ACTION_BASE_FEE_STORAGE_KEY)
            .await
            .context("failed reading sequence action base fee from state")?
            .ok_or_else(|| anyhow!("sequence action base fee not found"))?
            .0)
    }

    #[instrument(skip_all)]
    async fn get_sequence_action_byte_cost_multiplier(&self) -> Result<u128> {
        Ok(self
            .get::<_, Fee>(SEQUENCE_ACTION_BYTE_COST_MULTIPLIER_STORAGE_KEY)
            .await
            .context("failed reading raw sequence action byte cost multiplier from state")?
            .ok_or_else(|| anyhow!("sequence action byte cost multiplier not found"))?
            .0)
    }
}

impl<T: StateRead + ?Sized> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_sequence_action_base_fee(&self, fee: u128) {
        self.put(SEQUENCE_ACTION_BASE_FEE_STORAGE_KEY, Fee(fee));
    }

    #[instrument(skip_all)]
    fn put_sequence_action_byte_cost_multiplier(&self, fee: u128) {
        self.put(SEQUENCE_ACTION_BYTE_COST_MULTIPLIER_STORAGE_KEY, Fee(fee));
    }
}

impl<T: StateWrite> StateWriteExt for T {}

#[cfg(test)]
mod test {
    use super::{
        StateReadExt as _,
        StateWriteExt as _,
    };
    use crate::storage::Storage;

    #[tokio::test]
    async fn sequence_action_base_fee() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        let fee = 42;
        state.put_sequence_action_base_fee(fee);
        assert_eq!(state.get_sequence_action_base_fee().await.unwrap(), fee);
    }

    #[tokio::test]
    async fn sequence_action_byte_cost_multiplier() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        let fee = 42;
        state.put_sequence_action_byte_cost_multiplier(fee);
        assert_eq!(
            state
                .get_sequence_action_byte_cost_multiplier()
                .await
                .unwrap(),
            fee
        );
    }
}
