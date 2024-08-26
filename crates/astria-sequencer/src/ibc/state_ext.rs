use anyhow::{
    Context,
    Result,
};
use astria_core::{
    primitive::v1::{
        asset,
        asset::{
            IbcPrefixed,
            TracePrefixed,
        },
        RollupId,
        ADDRESS_LEN,
    },
    sequencerblock::v1alpha1::block::Deposit,
};
use async_trait::async_trait;
use ibc_types::core::channel::ChannelId;
use tendermint::Time;
use tracing::instrument;

use crate::storage::{
    BlockHeight,
    BlockTimestamp,
    ChainId,
    RevisionNumber,
    StoredValue,
};
use crate::{
    accounts::AddressBytes,
    storage::{
        self,
        Balance,
        Fee,
        // StateRead,
        // StateWrite,
    },
};

const IBC_SUDO_STORAGE_KEY: &str = "ibcsudo";
pub(crate) const ICS20_WITHDRAWAL_BASE_FEE_STORAGE_KEY: &str = "ics20withdrawalfee";

struct IbcRelayerKey<'a, T>(&'a T);

impl<'a, T: AddressBytes> std::fmt::Display for IbcRelayerKey<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ibc-relayer")?;
        f.write_str("/")?;
        for byte in self.0.address_bytes() {
            f.write_fmt(format_args!("{byte:02x}"))?;
        }
        Ok(())
    }
}

fn channel_balance_storage_key<TAsset: Into<asset::IbcPrefixed>>(
    channel: &ChannelId,
    asset: TAsset,
) -> String {
    format!(
        "ibc-data/{channel}/balance/{}",
        crate::storage_keys::hunks::Asset::from(asset),
    )
}

fn ibc_relayer_key<T: AddressBytes>(address: &T) -> String {
    IbcRelayerKey(address).to_string()
}

#[async_trait]
pub(crate) trait StateReadExt: cnidarium::StateRead {
    #[instrument(skip_all)]
    async fn get_ibc_channel_balance<TAsset>(
        &self,
        channel: &ChannelId,
        asset: TAsset,
    ) -> Result<u128>
    where
        TAsset: Into<asset::IbcPrefixed> + Send,
    {
        Ok(
            storage::get(self, channel_balance_storage_key(channel, asset))
                .await
                .context("failed reading ibc channel balance from state")?
                .map(Balance::try_from)
                .transpose()
                .context("failed parsing ibc channel balance from state")?
                .unwrap_or_default()
                .0,
        )
    }

    #[instrument(skip_all)]
    async fn get_ibc_sudo_address(&self) -> Result<[u8; ADDRESS_LEN]> {
        Ok(storage::get(self, IBC_SUDO_STORAGE_KEY)
            .await
            .context("failed reading ibc sudo key from state")?
            .map(storage::AddressBytes::try_from)
            .context("ibc sudo key not found")?
            .context("failed parsing ibc sudo key from state")?
            .0)
    }

    #[instrument(skip_all)]
    async fn is_ibc_relayer<T: AddressBytes>(&self, address: T) -> Result<bool> {
        Ok(storage::get(self, ibc_relayer_key(&address))
            .await
            .context("failed to read ibc relayer key from state")?
            .is_some())
    }

    #[instrument(skip_all)]
    async fn get_ics20_withdrawal_base_fee(&self) -> Result<u128> {
        Ok(storage::get(self, ICS20_WITHDRAWAL_BASE_FEE_STORAGE_KEY)
            .await
            .context("failed reading ics20 withdrawal fee from state")?
            .map(Fee::try_from)
            .context("ics20 withdrawal fee not found")?
            .context("failed parsing ics20 withdrawal fee from state")?
            .0)
    }

    // =============================================================================================

    #[instrument(skip_all)]
    async fn map_ibc_to_trace_prefixed_asset(
        &self,
        asset: IbcPrefixed,
    ) -> Result<Option<TracePrefixed>> {
        storage::get(self, &crate::assets::asset_storage_key(asset))
            .await
            .context("failed reading asset from state")?
            .map(TracePrefixed::try_from)
            .transpose()
    }

    #[instrument(skip_all)]
    async fn has_ibc_asset<TAsset: Into<IbcPrefixed> + Send>(&self, asset: TAsset) -> Result<bool> {
        Ok(storage::get(self, &crate::assets::asset_storage_key(asset))
            .await
            .context("failed reading asset from state")?
            .is_some())
    }

    #[instrument(skip_all)]
    async fn get_bridge_account_rollup_id<T: AddressBytes>(
        &self,
        address: T,
    ) -> Result<Option<RollupId>> {
        storage::get(self, &crate::bridge::rollup_id_storage_key(&address))
            .await
            .context("failed reading bridge account rollup ID from state")?
            .map(RollupId::try_from)
            .transpose()
    }

    #[instrument(skip_all)]
    async fn get_bridge_account_ibc_asset<T: AddressBytes>(
        &self,
        address: T,
    ) -> Result<IbcPrefixed> {
        IbcPrefixed::try_from(
            storage::get(self, &crate::bridge::asset_id_storage_key(&address))
                .await
                .context("failed reading asset ID from state")?
                .context("asset ID not found")?,
        )
        .context("failed parsing asset ID from state")
    }

    #[instrument(skip_all)]
    async fn get_chain_id(&self) -> Result<tendermint::chain::Id> {
        tendermint::chain::Id::try_from(
            ChainId::try_from(
                storage::get(self, crate::state_ext::CHAIN_ID_KEY)
                    .await
                    .context("failed to read chain_id from state")?
                    .context("chain id not found in state")?,
            )?
            .0,
        )
        .context("invalid chain id from state")
    }

    #[instrument(skip_all)]
    async fn get_revision_number(&self) -> Result<u64> {
        Ok(RevisionNumber::try_from(
            storage::get(self, crate::state_ext::REVISION_NUMBER_KEY)
                .await
                .context("failed to read revision number from state")?
                .context("revision number not found in state")?,
        )
        .context("failed to parse revision number from state")?
        .0)
    }

    #[instrument(skip_all)]
    async fn get_block_height(&self) -> Result<u64> {
        Ok(BlockHeight::try_from(
            storage::get(self, crate::state_ext::BLOCK_HEIGHT_KEY)
                .await
                .context("failed to read block_height from state")?
                .context("block height not found in state")?,
        )
        .context("failed to parse block_height from state")?
        .0)
    }

    #[instrument(skip_all)]
    async fn get_block_timestamp(&self) -> Result<Time> {
        Ok(BlockTimestamp::try_from(
            storage::get(self, crate::state_ext::BLOCK_TIMESTAMP_KEY)
                .await
                .context("failed to read block_timestamp from state")?
                .context("block timestamp not found")?,
        )
        .context("failed to parse block_timestamp from state")?
        .0)
    }
}

impl<T: cnidarium::StateRead> StateReadExt for T {}

#[async_trait]
pub(crate) trait StateWriteExt: cnidarium::StateWrite {
    #[instrument(skip_all)]
    fn put_ibc_channel_balance<TAsset>(
        &mut self,
        channel: &ChannelId,
        asset: TAsset,
        balance: u128,
    ) -> Result<()>
    where
        TAsset: Into<asset::IbcPrefixed> + Send,
    {
        storage::put(
            self,
            channel_balance_storage_key(channel, asset),
            &StoredValue::Balance(Balance(balance)),
        )
    }

    #[instrument(skip_all)]
    fn put_ibc_sudo_address<T: AddressBytes>(&mut self, address: T) -> Result<()> {
        storage::put(
            self,
            IBC_SUDO_STORAGE_KEY,
            &StoredValue::AddressBytes(storage::AddressBytes(address.address_bytes())),
        )
    }

    #[instrument(skip_all)]
    fn put_ibc_relayer_address<T: AddressBytes>(&mut self, address: T) -> Result<()> {
        storage::put(self, ibc_relayer_key(&address), &StoredValue::Unit)
    }

    #[instrument(skip_all)]
    fn delete_ibc_relayer_address<T: AddressBytes>(&mut self, address: T) {
        storage::delete(self, ibc_relayer_key(&address));
    }

    #[instrument(skip_all)]
    fn put_ics20_withdrawal_base_fee(&mut self, fee: u128) -> Result<()> {
        storage::put(
            self,
            ICS20_WITHDRAWAL_BASE_FEE_STORAGE_KEY,
            &StoredValue::Fee(Fee(fee)),
        )
    }

    // =============================================================================================

    fn put_ibc_asset(&mut self, asset: TracePrefixed) -> Result<()> {
        storage::put(
            self,
            crate::assets::asset_storage_key(&asset),
            &StoredValue::TracePrefixedDenom(asset),
        )
    }

    #[instrument(skip_all)]
    async fn increase_balance<TAddress, TAsset>(
        &mut self,
        address: TAddress,
        asset: TAsset,
        amount: u128,
    ) -> Result<()>
    where
        TAddress: AddressBytes,
        TAsset: Into<asset::IbcPrefixed> + std::fmt::Display + Send,
    {
        let asset = asset.into();
        let storage_key = crate::accounts::balance_storage_key(address, asset);
        let current_balance = storage::get(self, &storage_key)
            .await
            .context("failed reading account balance from state")?
            .map(Balance::try_from)
            .transpose()
            .context("failed parsing account balance from state")?
            .unwrap_or_default()
            .0;

        let new_balance = current_balance
            .checked_add(amount)
            .context("failed to update account balance due to overflow")?;

        storage::put(
            self,
            storage_key,
            &StoredValue::Balance(Balance(new_balance)),
        )
    }

    #[instrument(skip_all)]
    async fn put_bridge_deposit(&mut self, deposit: Deposit) -> Result<()> {
        let mut deposits: Vec<Deposit> = self.object_get("deposits").unwrap_or_default();
        deposits.push(deposit);
        self.object_put("deposits", deposits);
        Ok(())
    }
}

impl<T: cnidarium::StateWrite> StateWriteExt for T {}

// #[cfg(test)]
// mod tests {
//     use astria_core::primitive::v1::{
//         asset,
//         Address,
//     };
//     use ibc_types::core::channel::ChannelId;
//     use insta::assert_snapshot;
//
//     use super::{
//         StateReadExt as _,
//         StateWriteExt as _,
//     };
//     use crate::{
//         address::StateWriteExt,
//         ibc::state_ext::channel_balance_storage_key,
//         storage::Storage,
//         test_utils::{
//             astria_address,
//             ASTRIA_PREFIX,
//         },
//     };
//
//     fn asset_0() -> asset::Denom {
//         "asset_0".parse().unwrap()
//     }
//     fn asset_1() -> asset::Denom {
//         "asset_1".parse().unwrap()
//     }
//
//     #[tokio::test]
//     async fn get_ibc_sudo_address_fails_if_not_set() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         // should fail if not set
//         state
//             .get_ibc_sudo_address()
//             .await
//             .expect_err("sudo address should be set");
//     }
//
//     #[tokio::test]
//     async fn put_ibc_sudo_address() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         state.put_base_prefix(ASTRIA_PREFIX).unwrap();
//
//         // can write new
//         let mut address = [42u8; 20];
//         state.put_ibc_sudo_address(address);
//         assert_eq!(
//             state
//                 .get_ibc_sudo_address()
//                 .await
//                 .expect("a sudo address was written and must exist inside the database"),
//             address,
//             "stored sudo address was not what was expected"
//         );
//
//         // can rewrite with new value
//         address = [41u8; 20];
//         state.put_ibc_sudo_address(address);
//         assert_eq!(
//             state
//                 .get_ibc_sudo_address()
//                 .await
//                 .expect("sudo address was written and must exist inside the database"),
//             address,
//             "updated sudo address was not what was expected"
//         );
//     }
//
//     #[tokio::test]
//     async fn is_ibc_relayer_ok_if_not_set() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         state.put_base_prefix(ASTRIA_PREFIX).unwrap();
//
//         // unset address returns false
//         let address = astria_address(&[42u8; 20]);
//         assert!(
//             !state
//                 .is_ibc_relayer(address)
//                 .await
//                 .expect("calls to properly formatted addresses should not fail"),
//             "inputted address should've returned false"
//         );
//     }
//
//     #[tokio::test]
//     async fn delete_ibc_relayer_address() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         state.put_base_prefix(ASTRIA_PREFIX).unwrap();
//
//         // can write
//         let address = astria_address(&[42u8; 20]);
//         state.put_ibc_relayer_address(address);
//         assert!(
//             state
//                 .is_ibc_relayer(address)
//                 .await
//                 .expect("a relayer address was written and must exist inside the database"),
//             "stored relayer address could not be verified"
//         );
//
//         // can delete
//         state.delete_ibc_relayer_address(address);
//         assert!(
//             !state
//                 .is_ibc_relayer(address)
//                 .await
//                 .expect("calls on unset addresses should not fail"),
//             "relayer address was not deleted as was intended"
//         );
//     }
//
//     #[tokio::test]
//     async fn put_ibc_relayer_address() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         state.put_base_prefix(ASTRIA_PREFIX).unwrap();
//
//         // can write
//         let address = astria_address(&[42u8; 20]);
//         state.put_ibc_relayer_address(address);
//         assert!(
//             state
//                 .is_ibc_relayer(address)
//                 .await
//                 .expect("a relayer address was written and must exist inside the database"),
//             "stored relayer address could not be verified"
//         );
//
//         // can write multiple
//         let address_1 = astria_address(&[41u8; 20]);
//         state.put_ibc_relayer_address(address_1);
//         assert!(
//             state
//                 .is_ibc_relayer(address_1)
//                 .await
//                 .expect("a relayer address was written and must exist inside the database"),
//             "additional stored relayer address could not be verified"
//         );
//         assert!(
//             state
//                 .is_ibc_relayer(address)
//                 .await
//                 .expect("a relayer address was written and must exist inside the database"),
//             "original stored relayer address could not be verified"
//         );
//     }
//
//     #[tokio::test]
//     async fn get_ibc_channel_balance_unset_ok() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         let channel = ChannelId::new(0u64);
//         let asset = asset_0();
//
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel, asset)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             0u128,
//             "unset asset and channel should return zero"
//         );
//     }
//
//     #[tokio::test]
//     async fn put_ibc_channel_balance_simple() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         let channel = ChannelId::new(0u64);
//         let asset = asset_0();
//         let mut amount = 10u128;
//
//         // write initial
//         state.put_ibc_channel_balance(&channel, &asset, amount);
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel, &asset)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             amount,
//             "set balance for channel/asset pair not what was expected"
//         );
//
//         // can update
//         amount = 20u128;
//         state.put_ibc_channel_balance(&channel, &asset, amount);
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel, &asset)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             amount,
//             "set balance for channel/asset pair not what was expected"
//         );
//     }
//
//     #[tokio::test]
//     async fn put_ibc_channel_balance_multiple_assets() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         let channel = ChannelId::new(0u64);
//         let asset_0 = asset_0();
//         let asset_1 = asset_1();
//         let amount_0 = 10u128;
//         let amount_1 = 20u128;
//
//         // write both
//         state.put_ibc_channel_balance(&channel, &asset_0, amount_0);
//         state.put_ibc_channel_balance(&channel, &asset_1, amount_1);
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel, &asset_0)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             amount_0,
//             "set balance for channel/asset pair not what was expected"
//         );
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel, &asset_1)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             amount_1,
//             "set balance for channel/asset pair not what was expected"
//         );
//     }
//
//     #[tokio::test]
//     async fn put_ibc_channel_balance_multiple_channels() {
//         let storage = Storage::new_temp().await;
//         let state = storage.new_delta_of_latest_snapshot();
//
//         let channel_0 = ChannelId::new(0u64);
//         let channel_1 = ChannelId::new(1u64);
//         let asset = asset_0();
//         let amount_0 = 10u128;
//         let amount_1 = 20u128;
//
//         // write both
//         state.put_ibc_channel_balance(&channel_0, &asset, amount_0);
//         state.put_ibc_channel_balance(&channel_1, &asset, amount_1);
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel_0, &asset)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             amount_0,
//             "set balance for channel/asset pair not what was expected"
//         );
//         assert_eq!(
//             state
//                 .get_ibc_channel_balance(&channel_1, asset)
//                 .await
//                 .expect("retrieving asset balance for channel should not fail"),
//             amount_1,
//             "set balance for channel/asset pair not what was expected"
//         );
//     }
//
//     #[test]
//     fn storage_keys_have_not_changed() {
//         let channel = ChannelId::new(5);
//         let address: Address = "astria1rsxyjrcm255ds9euthjx6yc3vrjt9sxrm9cfgm"
//             .parse()
//             .unwrap();
//
//         assert_snapshot!(super::ibc_relayer_key(&address));
//
//         let asset = "an/asset/with/a/prefix"
//             .parse::<astria_core::primitive::v1::asset::Denom>()
//             .unwrap();
//         assert_eq!(
//             channel_balance_storage_key(&channel, &asset),
//             channel_balance_storage_key(&channel, asset.to_ibc_prefixed()),
//         );
//         assert_snapshot!(channel_balance_storage_key(&channel, &asset));
//     }
// }
