use anyhow::{
    bail,
    Context,
    Result,
};
use astria_core::primitive::v1::ADDRESS_LEN;
use async_trait::async_trait;
use tracing::instrument;

use super::ValidatorSet;
use crate::{
    accounts::AddressBytes,
    storage::{
        self,
        Fee,
        StateRead,
        StateWrite,
    },
};

const SUDO_STORAGE_KEY: &str = "sudo";
const VALIDATOR_SET_STORAGE_KEY: &str = "valset";
const VALIDATOR_UPDATES_KEY: &[u8] = b"valupdates";

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    #[instrument(skip_all)]
    async fn get_sudo_address(&self) -> Result<[u8; ADDRESS_LEN]> {
        let Some(sudo_address) = self
            .get::<_, storage::AddressBytes>(SUDO_STORAGE_KEY)
            .await
            .context("failed reading sudo key from state")?
        else {
            bail!("sudo key not found");
        };
        Ok(sudo_address.0)
    }

    #[instrument(skip_all)]
    async fn get_validator_set(&self) -> Result<ValidatorSet> {
        self.get(VALIDATOR_SET_STORAGE_KEY)
            .await
            .transpose()
            .context("validator set not found")?
            .context("failed reading validator set from state")
    }

    #[instrument(skip_all)]
    async fn get_validator_updates(&self) -> Result<ValidatorSet> {
        Ok(self
            .nonverifiable_get(VALIDATOR_UPDATES_KEY)
            .await
            .context("failed reading raw validator updates from state")?
            .unwrap_or_default()) // Return empty set because validator updates are optional.
    }
}

impl<T: StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    fn put_sudo_address<T: AddressBytes>(&self, address: T) {
        self.put(
            SUDO_STORAGE_KEY,
            storage::AddressBytes(address.address_bytes()),
        );
    }

    fn put_validator_set(&self, validator_set: ValidatorSet) {
        self.put(VALIDATOR_SET_STORAGE_KEY, validator_set);
    }

    fn put_validator_updates(&self, validator_updates: ValidatorSet) {
        self.nonverifiable_put(VALIDATOR_UPDATES_KEY, validator_updates);
    }

    fn clear_validator_updates(&self) {
        self.nonverifiable_delete(VALIDATOR_UPDATES_KEY);
    }

    #[instrument(skip_all)]
    fn put_ics20_withdrawal_base_fee(&self, fee: u128) {
        self.put(crate::ibc::ICS20_WITHDRAWAL_BASE_FEE_STORAGE_KEY, Fee(fee));
    }
}

impl<T: StateWrite> StateWriteExt for T {}

#[cfg(test)]
mod tests {
    use astria_core::{
        primitive::v1::ADDRESS_LEN,
        protocol::transaction::v1alpha1::action::ValidatorUpdate,
    };

    use super::{
        StateReadExt as _,
        StateWriteExt as _,
    };
    use crate::{
        address::StateWriteExt as _,
        authority::ValidatorSet,
        storage::Storage,
        test_utils::{
            verification_key,
            ASTRIA_PREFIX,
        },
    };

    fn empty_validator_set() -> ValidatorSet {
        ValidatorSet::new_from_updates(vec![])
    }

    #[tokio::test]
    async fn sudo_address() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        state.put_base_prefix(ASTRIA_PREFIX).unwrap();

        // doesn't exist at first
        state
            .get_sudo_address()
            .await
            .expect_err("no sudo address should exist at first");

        // can write new
        let mut address_expected = [42u8; ADDRESS_LEN];
        state.put_sudo_address(address_expected);
        assert_eq!(
            state
                .get_sudo_address()
                .await
                .expect("a sudo address was written and must exist inside the database"),
            address_expected,
            "stored sudo address was not what was expected"
        );

        // can rewrite with new value
        address_expected = [41u8; ADDRESS_LEN];
        state.put_sudo_address(address_expected);
        assert_eq!(
            state
                .get_sudo_address()
                .await
                .expect("a new sudo address was written and must exist inside the database"),
            address_expected,
            "updated sudo address was not what was expected"
        );
    }

    #[tokio::test]
    async fn validator_set_uninitialized_fails() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // doesn't exist at first
        state
            .get_validator_set()
            .await
            .expect_err("no validator set should exist at first");
    }

    #[tokio::test]
    async fn put_validator_set() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        let initial = vec![ValidatorUpdate {
            power: 10,
            verification_key: verification_key(1),
        }];
        let initial_validator_set = ValidatorSet::new_from_updates(initial);

        // can write new
        state.put_validator_set(initial_validator_set.clone());
        assert_eq!(
            state
                .get_validator_set()
                .await
                .expect("a validator set was written and must exist inside the database"),
            initial_validator_set,
            "stored validator set was not what was expected"
        );

        // can update
        let updates = vec![ValidatorUpdate {
            power: 20,
            verification_key: verification_key(2),
        }];
        let updated_validator_set = ValidatorSet::new_from_updates(updates);
        state.put_validator_set(updated_validator_set.clone());
        assert_eq!(
            state
                .get_validator_set()
                .await
                .expect("a validator set was written and must exist inside the database"),
            updated_validator_set,
            "stored validator set was not what was expected"
        );
    }

    #[tokio::test]
    async fn get_validator_updates_empty() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // querying for empty validator set is ok
        assert_eq!(
            state
                .get_validator_updates()
                .await
                .expect("if no updates have been written return empty set"),
            empty_validator_set(),
            "returned empty validator set different than expected"
        );
    }

    #[tokio::test]
    async fn put_validator_updates() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // create update validator set
        let mut updates = vec![
            ValidatorUpdate {
                power: 10,
                verification_key: verification_key(1),
            },
            ValidatorUpdate {
                power: 0,
                verification_key: verification_key(2),
            },
        ];
        let mut validator_set_updates = ValidatorSet::new_from_updates(updates);

        // put validator updates
        state.put_validator_updates(validator_set_updates.clone());
        assert_eq!(
            state
                .get_validator_updates()
                .await
                .expect("an update validator set was written and must exist inside the database"),
            validator_set_updates,
            "stored update validator set was not what was expected"
        );

        // create different updates
        updates = vec![
            ValidatorUpdate {
                power: 22,
                verification_key: verification_key(1),
            },
            ValidatorUpdate {
                power: 10,
                verification_key: verification_key(3),
            },
        ];

        validator_set_updates = ValidatorSet::new_from_updates(updates);

        // write different updates
        state.put_validator_updates(validator_set_updates.clone());
        assert_eq!(
            state
                .get_validator_updates()
                .await
                .expect("an update validator set was written and must exist inside the database"),
            validator_set_updates,
            "stored update validator set was not what was expected"
        );
    }

    #[tokio::test]
    async fn clear_validator_updates() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // create update validator set
        let updates = vec![ValidatorUpdate {
            power: 10,
            verification_key: verification_key(1),
        }];
        let validator_set_updates = ValidatorSet::new_from_updates(updates);

        // put validator updates
        state.put_validator_updates(validator_set_updates.clone());
        assert_eq!(
            state
                .get_validator_updates()
                .await
                .expect("an update validator set was written and must exist inside the database"),
            validator_set_updates,
            "stored update validator set was not what was expected"
        );

        // clear updates
        state.clear_validator_updates();

        // check that clear worked
        assert_eq!(
            state
                .get_validator_updates()
                .await
                .expect("if no updates have been written return empty set"),
            empty_validator_set(),
            "returned validator set different than expected"
        );
    }

    #[tokio::test]
    async fn clear_validator_updates_empty_ok() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        // able to clear non-existent updates with no error
        state.clear_validator_updates();
    }

    #[tokio::test]
    async fn execute_validator_updates() {
        // create initial validator set
        let initial = vec![
            ValidatorUpdate {
                power: 1,
                verification_key: verification_key(0),
            },
            ValidatorUpdate {
                power: 2,
                verification_key: verification_key(1),
            },
            ValidatorUpdate {
                power: 3,
                verification_key: verification_key(2),
            },
        ];
        let mut initial_validator_set = ValidatorSet::new_from_updates(initial);

        // create set of updates (update key_0, remove key_1)
        let updates = vec![
            ValidatorUpdate {
                power: 5,
                verification_key: verification_key(0),
            },
            ValidatorUpdate {
                power: 0,
                verification_key: verification_key(1),
            },
        ];

        let validator_set_updates = ValidatorSet::new_from_updates(updates);

        // apply updates
        initial_validator_set.apply_updates(validator_set_updates);

        // create end state
        let updates = vec![
            ValidatorUpdate {
                power: 5,
                verification_key: verification_key(0),
            },
            ValidatorUpdate {
                power: 3,
                verification_key: verification_key(2),
            },
        ];
        let validator_set_endstate = ValidatorSet::new_from_updates(updates);

        // check updates applied correctly
        assert_eq!(
            initial_validator_set, validator_set_endstate,
            "validator set apply updates did not behave as expected"
        );
    }
}
