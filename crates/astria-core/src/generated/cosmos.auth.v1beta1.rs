/// BaseAccount defines a base account type. It contains all the necessary fields
/// for basic account functionality. Any custom account type should extend this
/// type for additional functionality (e.g. vesting).
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BaseAccount {
    #[prost(string, tag = "1")]
    pub address: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub pub_key: ::core::option::Option<::pbjson_types::Any>,
    #[prost(uint64, tag = "3")]
    pub account_number: u64,
    #[prost(uint64, tag = "4")]
    pub sequence: u64,
}
impl ::prost::Name for BaseAccount {
    const NAME: &'static str = "BaseAccount";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// Params defines the parameters for the auth module.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Params {
    #[prost(uint64, tag = "1")]
    pub max_memo_characters: u64,
    #[prost(uint64, tag = "2")]
    pub tx_sig_limit: u64,
    #[prost(uint64, tag = "3")]
    pub tx_size_cost_per_byte: u64,
    #[prost(uint64, tag = "4")]
    pub sig_verify_cost_ed25519: u64,
    #[prost(uint64, tag = "5")]
    pub sig_verify_cost_secp256k1: u64,
}
impl ::prost::Name for Params {
    const NAME: &'static str = "Params";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// QueryAccountsRequest is the request type for the Query/Accounts RPC method.
///
/// Since: cosmos-sdk 0.43
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryAccountsRequest {
    /// pagination defines an optional pagination for the request.
    #[prost(message, optional, tag = "1")]
    pub pagination: ::core::option::Option<
        super::super::base::query::v1beta1::PageRequest,
    >,
}
impl ::prost::Name for QueryAccountsRequest {
    const NAME: &'static str = "QueryAccountsRequest";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// QueryAccountsResponse is the response type for the Query/Accounts RPC method.
///
/// Since: cosmos-sdk 0.43
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryAccountsResponse {
    /// accounts are the existing accounts
    #[prost(message, repeated, tag = "1")]
    pub accounts: ::prost::alloc::vec::Vec<::pbjson_types::Any>,
    /// pagination defines the pagination in the response.
    #[prost(message, optional, tag = "2")]
    pub pagination: ::core::option::Option<
        super::super::base::query::v1beta1::PageResponse,
    >,
}
impl ::prost::Name for QueryAccountsResponse {
    const NAME: &'static str = "QueryAccountsResponse";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// QueryAccountRequest is the request type for the Query/Account RPC method.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryAccountRequest {
    /// address defines the address to query for.
    #[prost(string, tag = "1")]
    pub address: ::prost::alloc::string::String,
}
impl ::prost::Name for QueryAccountRequest {
    const NAME: &'static str = "QueryAccountRequest";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// QueryAccountResponse is the response type for the Query/Account RPC method.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryAccountResponse {
    /// account defines the account of the corresponding address.
    #[prost(message, optional, tag = "1")]
    pub account: ::core::option::Option<::pbjson_types::Any>,
}
impl ::prost::Name for QueryAccountResponse {
    const NAME: &'static str = "QueryAccountResponse";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// QueryParamsRequest is the request type for the Query/Params RPC method.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryParamsRequest {}
impl ::prost::Name for QueryParamsRequest {
    const NAME: &'static str = "QueryParamsRequest";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// QueryParamsResponse is the response type for the Query/Params RPC method.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryParamsResponse {
    /// params defines the parameters of the module.
    #[prost(message, optional, tag = "1")]
    pub params: ::core::option::Option<Params>,
}
impl ::prost::Name for QueryParamsResponse {
    const NAME: &'static str = "QueryParamsResponse";
    const PACKAGE: &'static str = "cosmos.auth.v1beta1";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("cosmos.auth.v1beta1.{}", Self::NAME)
    }
}
/// Generated client implementations.
#[cfg(feature = "client")]
pub mod query_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Query defines the gRPC querier service.
    #[derive(Debug, Clone)]
    pub struct QueryClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl QueryClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> QueryClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> QueryClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            QueryClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        /// Accounts returns all the existing accounts
        ///
        /// Since: cosmos-sdk 0.43
        pub async fn accounts(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryAccountsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryAccountsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/cosmos.auth.v1beta1.Query/Accounts",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("cosmos.auth.v1beta1.Query", "Accounts"));
            self.inner.unary(req, path, codec).await
        }
        /// Account returns account details based on address.
        pub async fn account(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryAccountRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryAccountResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/cosmos.auth.v1beta1.Query/Account",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("cosmos.auth.v1beta1.Query", "Account"));
            self.inner.unary(req, path, codec).await
        }
        /// Params queries all parameters.
        pub async fn params(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryParamsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryParamsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/cosmos.auth.v1beta1.Query/Params",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("cosmos.auth.v1beta1.Query", "Params"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
#[cfg(feature = "server")]
pub mod query_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with QueryServer.
    #[async_trait]
    pub trait Query: Send + Sync + 'static {
        /// Accounts returns all the existing accounts
        ///
        /// Since: cosmos-sdk 0.43
        async fn accounts(
            self: std::sync::Arc<Self>,
            request: tonic::Request<super::QueryAccountsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryAccountsResponse>,
            tonic::Status,
        >;
        /// Account returns account details based on address.
        async fn account(
            self: std::sync::Arc<Self>,
            request: tonic::Request<super::QueryAccountRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryAccountResponse>,
            tonic::Status,
        >;
        /// Params queries all parameters.
        async fn params(
            self: std::sync::Arc<Self>,
            request: tonic::Request<super::QueryParamsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryParamsResponse>,
            tonic::Status,
        >;
    }
    /// Query defines the gRPC querier service.
    #[derive(Debug)]
    pub struct QueryServer<T: Query> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Query> QueryServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for QueryServer<T>
    where
        T: Query,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/cosmos.auth.v1beta1.Query/Accounts" => {
                    #[allow(non_camel_case_types)]
                    struct AccountsSvc<T: Query>(pub Arc<T>);
                    impl<
                        T: Query,
                    > tonic::server::UnaryService<super::QueryAccountsRequest>
                    for AccountsSvc<T> {
                        type Response = super::QueryAccountsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryAccountsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Query>::accounts(inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AccountsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/cosmos.auth.v1beta1.Query/Account" => {
                    #[allow(non_camel_case_types)]
                    struct AccountSvc<T: Query>(pub Arc<T>);
                    impl<
                        T: Query,
                    > tonic::server::UnaryService<super::QueryAccountRequest>
                    for AccountSvc<T> {
                        type Response = super::QueryAccountResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryAccountRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Query>::account(inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AccountSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/cosmos.auth.v1beta1.Query/Params" => {
                    #[allow(non_camel_case_types)]
                    struct ParamsSvc<T: Query>(pub Arc<T>);
                    impl<T: Query> tonic::server::UnaryService<super::QueryParamsRequest>
                    for ParamsSvc<T> {
                        type Response = super::QueryParamsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryParamsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Query>::params(inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ParamsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: Query> Clone for QueryServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: Query> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Query> tonic::server::NamedService for QueryServer<T> {
        const NAME: &'static str = "cosmos.auth.v1beta1.Query";
    }
}
