pub(crate) mod query;
pub(crate) mod state_ext;

use std::sync::OnceLock;

use astria_core::primitive::v1::asset::Denom;

pub(crate) static NATIVE_ASSET: OnceLock<Denom> = OnceLock::new();

pub(crate) fn initialize_native_asset(native_asset: &str) {
    if NATIVE_ASSET.get().is_some() {
        tracing::error!("native asset should only be set once");
        return;
    }

    let denom = native_asset
        .parse::<Denom>()
        .expect("being unable to parse the native asset breaks sequencer");
    NATIVE_ASSET
        .set(denom)
        .expect("native asset should only be set once");
}

pub(crate) fn get_native_asset() -> &'static Denom {
    NATIVE_ASSET.get().expect("native asset should be set")
}
