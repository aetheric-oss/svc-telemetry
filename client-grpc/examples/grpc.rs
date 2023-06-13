//! gRPC client implementation

use lib_common::grpc::get_endpoint_from_env;
use svc_telemetry_client_grpc::client::{ReadyRequest, RpcServiceClient};
use svc_telemetry_client_grpc::{Client, ClientConnect, GrpcClient};
use tonic::transport::Channel;

/// Example svc-telemetry-client-grpc
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "telemetry");
    println!("Connection created");
    println!(
        "NOTE: Ensure the server is running on {} or this example will fail.",
        connection.get_address()
    );

    let response = connection
        .get_client()
        .await?
        .is_ready(tonic::Request::new(ReadyRequest {}))
        .await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
