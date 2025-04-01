/// A JSON-encoded form of this message is used as the upgrades file for the Sequencer.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Upgrades {
    #[prost(message, optional, tag = "1")]
    pub aspen: ::core::option::Option<Aspen>,
}
impl ::prost::Name for Upgrades {
    const NAME: &'static str = "Upgrades";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
/// Info specific to a given upgrade.
///
/// All upgrades have this info at a minimum.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BaseUpgradeInfo {
    /// The upgrade should be applied during the lifecycle of the block at this height.
    #[prost(uint64, tag = "1")]
    pub activation_height: u64,
    /// The app version running after the upgrade is applied.
    #[prost(uint64, tag = "2")]
    pub app_version: u64,
}
impl ::prost::Name for BaseUpgradeInfo {
    const NAME: &'static str = "BaseUpgradeInfo";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
/// Aspen upgrade of the Sequencer network.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Aspen {
    #[prost(message, optional, tag = "1")]
    pub base_info: ::core::option::Option<BaseUpgradeInfo>,
    #[prost(message, optional, tag = "2")]
    pub price_feed_change: ::core::option::Option<aspen::PriceFeedChange>,
    #[prost(message, optional, tag = "3")]
    pub validator_update_action_change: ::core::option::Option<
        aspen::ValidatorUpdateActionChange,
    >,
}
/// Nested message and enum types in `Aspen`.
pub mod aspen {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct PriceFeedChange {
        /// The price feed genesis data.
        #[prost(message, optional, tag = "1")]
        pub genesis: ::core::option::Option<
            super::super::super::protocol::genesis::v1::PriceFeedGenesis,
        >,
    }
    impl ::prost::Name for PriceFeedChange {
        const NAME: &'static str = "PriceFeedChange";
        const PACKAGE: &'static str = "astria.upgrades.v1";
        fn full_name() -> ::prost::alloc::string::String {
            ::prost::alloc::format!("astria.upgrades.v1.Aspen.{}", Self::NAME)
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct ValidatorUpdateActionChange {}
    impl ::prost::Name for ValidatorUpdateActionChange {
        const NAME: &'static str = "ValidatorUpdateActionChange";
        const PACKAGE: &'static str = "astria.upgrades.v1";
        fn full_name() -> ::prost::alloc::string::String {
            ::prost::alloc::format!("astria.upgrades.v1.Aspen.{}", Self::NAME)
        }
    }
}
impl ::prost::Name for Aspen {
    const NAME: &'static str = "Aspen";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
