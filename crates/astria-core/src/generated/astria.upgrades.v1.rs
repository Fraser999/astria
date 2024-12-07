#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Upgrades {
    #[prost(message, optional, tag = "1")]
    pub connect_oracle: ::core::option::Option<ConnectOracleUpgrade>,
    #[prost(message, optional, tag = "2")]
    pub whatever: ::core::option::Option<WhateverUpgrade>,
}
impl ::prost::Name for Upgrades {
    const NAME: &'static str = "Upgrades";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BaseInfo {
    /// The upgrade should be applied during the lifecycle of the block at this height.
    #[prost(uint64, tag = "1")]
    pub activation_height: u64,
    /// Whether or not the sequencer should shut down after committing the block immediately
    /// before the activation height.
    #[prost(bool, tag = "2")]
    pub shutdown_required: bool,
}
impl ::prost::Name for BaseInfo {
    const NAME: &'static str = "BaseInfo";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectOracleUpgrade {
    #[prost(message, optional, tag = "1")]
    pub base_info: ::core::option::Option<BaseInfo>,
    #[prost(message, optional, tag = "2")]
    pub genesis: ::core::option::Option<
        super::super::protocol::genesis::v1::ConnectGenesis,
    >,
}
impl ::prost::Name for ConnectOracleUpgrade {
    const NAME: &'static str = "ConnectOracleUpgrade";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WhateverUpgrade {
    #[prost(message, optional, tag = "1")]
    pub base_info: ::core::option::Option<BaseInfo>,
}
impl ::prost::Name for WhateverUpgrade {
    const NAME: &'static str = "WhateverUpgrade";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
