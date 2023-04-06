//! gRPC server implementation

pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}

use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
use grpc_server::{ReadyRequest, ReadyResponse};

use crate::config::Config;
use crate::shutdown_signal;

use std::fmt::Debug;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct GrpcServerImpl {}

#[tonic::async_trait]
impl RpcService for GrpcServerImpl {
    /// Returns true when service is available
    #[cfg(not(tarpaulin_include))]
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}
/// Starts the grpc servers for this microservice using the provided configuration
///
/// # Example:
/// ```
/// use svc_telemetry::grpc::server::grpc_server;
/// use svc_telemetry::config::Config;
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::default();
///     tokio::spawn(grpc_server(config)).await
/// }
/// ```
#[cfg(not(tarpaulin_include))]
pub async fn grpc_server(config: Config) -> Result<(), ()> {
    grpc_info!("(grpc_server) entry.");

    // GRPC Server
    let grpc_port = config.docker_port_grpc;

    let addr = format!("[::]:{}", grpc_port);
    let Ok(full_grpc_addr) = addr.parse() else {
        grpc_error!("(grpc_server) invalid address: {:?}, exiting.", addr);
        return Err(());
    };

    let imp = GrpcServerImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<RpcServiceServer<GrpcServerImpl>>()
        .await;

    //start server
    grpc_info!("(grpc) hosted at {}.", full_grpc_addr);
    let _ = Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(imp))
        .serve_with_shutdown(full_grpc_addr, shutdown_signal("grpc"))
        .await;

    Ok(())
}
