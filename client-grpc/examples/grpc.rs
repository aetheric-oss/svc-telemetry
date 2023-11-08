//! gRPC client implementation

use lib_common::grpc::get_endpoint_from_env;
use svc_telemetry_client_grpc::prelude::*;

/// Example svc-telemetry-client-grpc
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    let client = TelemetryClient::new_client(&host, port, "telemetry");
    println!("Client created.");
    println!(
        "NOTE: Ensure the server is running on {} or this example will fail.",
        client.get_address()
    );

    let response = client.is_ready(telemetry::ReadyRequest {}).await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
