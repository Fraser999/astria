use astria_core::{
    primitive::v1::asset,
    sequencer::Fees,
};

#[derive(Clone, Debug)]
pub(crate) struct ImmutableData {
    pub(crate) base_prefix: String,
    pub(crate) fees: Fees,
    pub(crate) native_asset: asset::TracePrefixed,
    pub(crate) chain_id: tendermint::chain::Id,
    pub(crate) authority_sudo_address: [u8; 20],
    pub(crate) ibc_sudo_address: [u8; 20],
}
