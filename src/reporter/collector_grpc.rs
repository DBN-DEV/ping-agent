#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Empty {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PingResult {
    #[prost(string, tag = "1")]
    pub ip: ::prost::alloc::string::String,
    #[prost(bool, tag = "2")]
    pub is_timeout: bool,
    #[prost(float, tag = "3")]
    pub rtt: f32,
    #[prost(string, tag = "4")]
    pub time: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TcpPingResult {
    #[prost(string, tag = "1")]
    pub target: ::prost::alloc::string::String,
    #[prost(bool, tag = "2")]
    pub is_timeout: bool,
    #[prost(float, tag = "3")]
    pub rtt: f32,
    #[prost(string, tag = "4")]
    pub send_at: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PingReportRequest {
    #[prost(message, optional, tag = "1")]
    pub result: ::core::option::Option<PingResult>,
    #[prost(uint32, tag = "2")]
    pub agent_id: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TcpPingReportRequest {
    #[prost(message, optional, tag = "1")]
    pub result: ::core::option::Option<TcpPingResult>,
    #[prost(uint32, tag = "2")]
    pub agent_id: u32,
}
#[doc = r" Generated client implementations."]
pub mod collector_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct CollectorClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl CollectorClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> CollectorClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn ping_report(
            &mut self,
            request: impl tonic::IntoRequest<super::PingReportRequest>,
        ) -> Result<tonic::Response<super::Empty>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/collector_grpc.Collector/PingReport");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn tcp_ping_report(
            &mut self,
            request: impl tonic::IntoRequest<super::TcpPingReportRequest>,
        ) -> Result<tonic::Response<super::Empty>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/collector_grpc.Collector/TcpPingReport");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for CollectorClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for CollectorClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "CollectorClient {{ ... }}")
        }
    }
}
