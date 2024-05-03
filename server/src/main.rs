//! Main function starting the server and initializing dependencies.

use grpc::server::grpc_server;
use lib_common::logger::load_logger_config_from_file;
use log::info;
use rest::server::rest_server;
use rest::{generate_openapi_spec, ApiDoc};
use svc_telemetry::*;

#[tokio::main]
#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) main entry point of the application
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("(main) server startup.");

    let config = Config::try_from_env()
        .map_err(|e| format!("Failed to load configuration from environment: {}", e))?;

    // Try to load log configuration from the provided log file.
    // Will default to stdout debug logging if the file can not be loaded.
    load_logger_config_from_file(config.log_config.as_str())
        .await
        .or_else(|e| Ok::<(), String>(log::error!("(main) {}", e)))?;
    info!("(main) Server startup.");

    // Allow option to only generate the spec file to a given location
    // use `make rust-openapi` to generate the OpenAPI specification
    let args = Cli::parse();
    if let Some(target) = args.openapi {
        return generate_openapi_spec::<ApiDoc>(&target).map_err(|e| e.into());
    }

    let grpc_clients = grpc::client::GrpcClients::default(config.clone());

    // REST Server
    tokio::spawn(rest_server(config.clone(), grpc_clients, None));

    // GRPC Server
    tokio::spawn(grpc_server(config, None)).await?;

    info!("(main) server shutdown.");

    // Make sure all log message are written/ displayed before shutdown
    log::logger().flush();
    Ok(())
}
