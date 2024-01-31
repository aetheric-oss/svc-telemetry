//! Main function starting the server and initializing dependencies.

use crate::grpc::{start_batch_loops, Batch, BatchLoop};
use log::info;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use svc_gis_client_grpc::prelude::gis::{AircraftId, AircraftPosition, AircraftVelocity};
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
    let position_ring = Arc::new(Mutex::new(VecDeque::<AircraftPosition>::with_capacity(
        n_items,
    )));

    let velocity_ring = Arc::new(Mutex::new(VecDeque::<AircraftVelocity>::with_capacity(
        n_items,
    )));

    let id_ring = Arc::new(Mutex::new(VecDeque::<AircraftId>::with_capacity(n_items)));

    // svc-gis dump
    start_batch_loops(
        id_ring.clone(),
        position_ring.clone(),
        velocity_ring.clone(),
        config.clone(),
    );
    let grpc_clients = grpc::client::GrpcClients::default(config.clone());

    // REST Server
    tokio::spawn(rest::server::rest_server(
        config.clone(),
        grpc_clients,
        id_ring.clone(),
        position_ring.clone(),
        velocity_ring.clone(),
        None,
    ));

    // GRPC Server
    tokio::spawn(grpc::server::grpc_server(config, None)).await?;

    info!("(main) server shutdown.");
    Ok(())
}
