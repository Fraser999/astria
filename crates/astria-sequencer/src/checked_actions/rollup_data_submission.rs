use astria_core::protocol::transaction::v1::action::RollupDataSubmission;
use astria_eyre::eyre::{
    ensure,
    Result,
};
use tracing::{
    instrument,
    Level,
};

#[derive(Debug)]
pub(crate) struct CheckedRollupDataSubmission {
    action: RollupDataSubmission,
}

impl CheckedRollupDataSubmission {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) fn new(action: RollupDataSubmission) -> Result<Self> {
        ensure!(
            !action.data.is_empty(),
            "cannot have empty data for sequence action"
        );

        let checked_action = Self {
            action,
        };

        Ok(checked_action)
    }
}

#[cfg(test)]
mod tests {
    use astria_core::primitive::v1::RollupId;
    use bytes::Bytes;

    use super::*;
    use crate::benchmark_and_test_utils::{
        assert_eyre_error,
        nria,
    };

    #[tokio::test]
    async fn should_fail_construction_if_data_is_empty() {
        let action = RollupDataSubmission {
            rollup_id: RollupId::new([1; 32]),
            data: Bytes::new(),
            fee_asset: nria().into(),
        };
        let err = CheckedRollupDataSubmission::new(action).unwrap_err();

        assert_eyre_error(&err, "cannot have empty data for sequence action");
    }
}
