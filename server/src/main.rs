//! Main function starting the server and initializing dependencies.

use log::info;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use svc_gis_client_grpc::prelude::gis::AircraftPosition;
use svc_telemetry::*;

#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("(main) server startup.");

    // Will use default config settings if no environment vars are found.
    let config = match Config::try_from_env() {
        Ok(config) => config,
        Err(e) => {
            panic!("(main) could not parse config from environment: {}.", e);
        }
    };

    // Start Logger
    let log_cfg: &str = config.log_config.as_str();
    if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
        panic!("(main) could not parse {}: {}.", log_cfg, e);
    }

    // Allow option to only generate the spec file to a given location
    // use `make rust-openapi` to generate the OpenAPI specification
    let args = Cli::parse();
    if let Some(target) = args.openapi {
        return rest::generate_openapi_spec(&target);
    }

    // Initialize Ring Buffer
    // Telemetry will also come over gRPC
    let n_items = config.ringbuffer_size_bytes as usize / std::mem::size_of::<AircraftPosition>();
    let ring = Arc::new(Mutex::new(VecDeque::<AircraftPosition>::with_capacity(
        n_items,
    )));

    // GRPC Clients
    let grpc_clients = grpc::client::GrpcClients::default(config.clone());

    //
    // Dump telemetry to svc-gis in bulk
    //
    tokio::spawn(gis::gis_batch_loop(
        grpc_clients.clone(),
        ring.clone(),
        config.gis_push_cadence_ms,
        config.gis_max_message_size_bytes,
    ));

    // REST Server
    tokio::spawn(rest::server::rest_server(
        config.clone(),
        grpc_clients,
        ring.clone(),
        None,
    ));

    // GRPC Server
    tokio::spawn(grpc::server::grpc_server(config, None)).await?;

    info!("(main) server shutdown.");
    Ok(())
}
