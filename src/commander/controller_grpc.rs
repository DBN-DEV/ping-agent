#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PingCommand {
    #[prost(string, tag = "1")]
    pub ip: ::prost::alloc::string::String,
    #[prost(uint32, tag = "2")]
    pub timeout_ms: u32,
    #[prost(uint32, tag = "3")]
    pub interval_ms: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PingCommandsResponse {
    #[prost(message, repeated, tag = "1")]
    pub commands: ::prost::alloc::vec::Vec<PingCommand>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandRequest {
    #[prost(uint32, tag = "1")]
    pub agent_id: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandCheckSumResponse {
    #[prost(string, tag = "1")]
    pub check_sum: ::prost::alloc::string::String,
}
#[doc = r" Generated client implementations."]
pub mod controller_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct ControllerClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ControllerClient<tonic::transport::Channel> {
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
    impl<T> ControllerClient<T>
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
        pub async fn get_command_check_sum(
            &mut self,
            request: impl tonic::IntoRequest<super::CommandRequest>,
        ) -> Result<tonic::Response<super::CommandCheckSumResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/controller_grpc.Controller/GetCommandCheckSum",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn get_ping_command(
            &mut self,
            request: impl tonic::IntoRequest<super::CommandRequest>,
        ) -> Result<tonic::Response<super::PingCommandsResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/controller_grpc.Controller/GetPingCommand");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for ControllerClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for ControllerClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "ControllerClient {{ ... }}")
        }
    }
}
