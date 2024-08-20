use anyhow::{
    bail,
    ensure,
    Context as _,
    Result,
};
use astria_core::primitive::v1::Address;
use async_trait::async_trait;
use tracing::instrument;

use crate::storage::{
    BasePrefix,
    StateRead,
    StateWrite,
};

fn base_prefix_key() -> &'static str {
    "prefixes/base"
}

#[async_trait]
pub(crate) trait StateReadExt: StateRead {
    async fn ensure_base_prefix(&self, address: &Address) -> anyhow::Result<()> {
        let prefix = self
            .get_base_prefix()
            .await
            .context("failed to read base prefix from state")?;
        ensure!(
            prefix == address.prefix(),
            "address has prefix `{}` but only `{prefix}` is permitted",
            address.prefix(),
        );
        Ok(())
    }

    async fn try_base_prefixed(&self, slice: &[u8]) -> anyhow::Result<Address> {
        let prefix = self
            .get_base_prefix()
            .await
            .context("failed to read base prefix from state")?;
        Address::builder()
            .slice(slice)
            .prefix(prefix)
            .try_build()
            .context("failed to construct address from byte slice and state-provided base prefix")
    }

    #[instrument(skip_all)]
    async fn get_base_prefix(&self) -> Result<String> {
        let Some(base_prefix) = self
            .get::<_, BasePrefix>(base_prefix_key())
            .await
            .context("failed reading address base prefix")?
        else {
            bail!("no base prefix found");
        };
        Ok(base_prefix.0)
    }
}

impl<T: ?Sized + StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: StateWrite {
    fn put_base_prefix(&self, prefix: &str) -> Result<()> {
        try_construct_dummy_address_from_prefix(prefix)
            .context("failed constructing a dummy address from the provided prefix")?;
        self.put(base_prefix_key(), BasePrefix(prefix.to_string()));
        Ok(())
    }
}

impl<T: StateWrite> StateWriteExt for T {}

fn try_construct_dummy_address_from_prefix(
    s: &str,
) -> Result<(), astria_core::primitive::v1::AddressError> {
    use astria_core::primitive::v1::ADDRESS_LEN;
    // construct a dummy address to see if we can construct it; fail otherwise.
    Address::builder()
        .array([0u8; ADDRESS_LEN])
        .prefix(s)
        .try_build()
        .map(|_| ())
}

#[cfg(test)]
mod test {
    use super::{
        StateReadExt as _,
        StateWriteExt as _,
    };
    use crate::storage::Storage;

    #[tokio::test]
    async fn put_and_get_base_prefix() {
        let storage = Storage::new_temp().await;
        let state = storage.new_delta_of_latest_snapshot();

        state.put_base_prefix("astria").unwrap();
        assert_eq!("astria", &state.get_base_prefix().await.unwrap());
    }
}
