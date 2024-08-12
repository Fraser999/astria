use async_trait::async_trait;
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};
use cnidarium::{
    StateRead,
    StateWrite,
};
use tracing::instrument;

use crate::immutable_data::ImmutableData;

const SEQUENCE_ACTION_BASE_FEE_STORAGE_KEY: &str = "seqbasefee";
const SEQUENCE_ACTION_BYTE_COST_MULTIPLIER_STORAGE_KEY: &str = "seqmultiplier";

/// Newtype wrapper to read and write a u128 from rocksdb.
#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct Fee(u128);

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    fn get_sequence_action_base_fee(&self, immutable_data: &ImmutableData) -> u128 {
        immutable_data.fees.sequence_base_fee
        // let bytes = self
        //     .get_raw(SEQUENCE_ACTION_BASE_FEE_STORAGE_KEY)
        //     .await
        //     .context("failed reading raw sequence action base fee from state")?
        //     .ok_or_else(|| anyhow!("sequence action base fee not found"))?;
        // let Fee(fee) = Fee::try_from_slice(&bytes).context("invalid fee bytes")?;
        // Ok(fee)
    }

    #[instrument(skip_all)]
    fn get_sequence_action_byte_cost_multiplier(&self, immutable_data: &ImmutableData) -> u128 {
        immutable_data.fees.sequence_byte_cost_multiplier
        // let bytes = self
        //     .get_raw(SEQUENCE_ACTION_BYTE_COST_MULTIPLIER_STORAGE_KEY)
        //     .await
        //     .context("failed reading raw sequence action byte cost multiplier from state")?
        //     .ok_or_else(|| anyhow!("sequence action byte cost multiplier not found"))?;
        // let Fee(fee) = Fee::try_from_slice(&bytes).context("invalid fee bytes")?;
        // Ok(fee)
    }
}

impl<T: StateRead + ?Sized> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    #[instrument(skip_all)]
    fn put_sequence_action_base_fee(&mut self, fee: u128) {
        self.put_raw(
            SEQUENCE_ACTION_BASE_FEE_STORAGE_KEY.to_string(),
            borsh::to_vec(&Fee(fee)).expect("failed to serialize fee"),
        );
    }

    #[instrument(skip_all)]
    fn put_sequence_action_byte_cost_multiplier(&mut self, fee: u128) {
        self.put_raw(
            SEQUENCE_ACTION_BYTE_COST_MULTIPLIER_STORAGE_KEY.to_string(),
            borsh::to_vec(&Fee(fee)).expect("failed to serialize fee"),
        );
    }
}

impl<T: StateWrite> StateWriteExt for T {}

// #[cfg(test)]
// mod test {
//     use cnidarium::StateDelta;
//
//     use super::{
//         StateReadExt as _,
//         StateWriteExt as _,
//     };
//
//     #[tokio::test]
//     async fn sequence_action_base_fee() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//
//         let fee = 42;
//         state.put_sequence_action_base_fee(fee);
//         assert_eq!(state.get_sequence_action_base_fee().await.unwrap(), fee);
//     }
//
//     #[tokio::test]
//     async fn sequence_action_byte_cost_multiplier() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//
//         let fee = 42;
//         state.put_sequence_action_byte_cost_multiplier(fee);
//         assert_eq!(
//             state
//                 .get_sequence_action_byte_cost_multiplier()
//                 .await
//                 .unwrap(),
//             fee
//         );
//     }
// }
