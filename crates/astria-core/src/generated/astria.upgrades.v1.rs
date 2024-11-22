#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Upgrade {
    #[prost(uint64, tag = "1")]
    pub activation_height: u64,
    #[prost(bool, tag = "2")]
    pub shutdown_required: bool,
    #[prost(message, repeated, tag = "3")]
    pub changes: ::prost::alloc::vec::Vec<Change>,
}
impl ::prost::Name for Upgrade {
    const NAME: &'static str = "Upgrade";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Change {
    #[prost(string, tag = "1")]
    pub description: ::prost::alloc::string::String,
    #[prost(uint64, optional, tag = "2")]
    pub activation_height: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "3")]
    pub value: ::core::option::Option<::pbjson_types::Any>,
}
impl ::prost::Name for Change {
    const NAME: &'static str = "Change";
    const PACKAGE: &'static str = "astria.upgrades.v1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("astria.upgrades.v1.{}", Self::NAME)
    }
}
