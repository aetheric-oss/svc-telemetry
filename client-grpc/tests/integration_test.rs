//! Integration Tests

use logtest::Logger;
use svc_telemetry_client_grpc::prelude::*;

const SERVICE_NAME: &str = "telemetry";

fn get_log_string(function: &str) -> String {
    #[cfg(feature = "stub_client")]
    return format!("({}) (MOCK) {} client.", function, SERVICE_NAME);

    #[cfg(not(feature = "stub_client"))]
    cfg_if::cfg_if! {
        if #[cfg(feature = "stub_backends")] {
            return format!("({}) (MOCK) {} server.", function, SERVICE_NAME);
        } else {
            return format!("({}) {} client.", function, SERVICE_NAME);
        }
    }
}

async fn test_is_ready(client: &TelemetryClient) {
    // Start the logger.
    let mut logger = Logger::start();

    let result = client.is_ready(telemetry::ReadyRequest {}).await;
    println!("{:?}", result);
    assert!(result.is_ok());

    // Search for the expected log message
    let expected = get_log_string("is_ready");
    println!("expected message: {}", expected);
    assert!(logger.any(|log| {
        if log.target().contains("app::") {
            println!("{}", log.target());
            let message = log.args();
            println!("{:?}", message);
            log.args() == expected
        } else {
            false
        }
    }));
}

#[tokio::test]
async fn test_grpc() {
    let (_, server_port) = lib_common::grpc::get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");

    let server_host = "web-server";
    let client = TelemetryClient::new_client(&server_host, server_port, SERVICE_NAME);

    test_is_ready(&client).await;
}
