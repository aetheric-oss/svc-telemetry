//! Main function starting the server and initializing dependencies.

mod cache;
mod config;
mod grpc;
mod rest;

use clap::Parser;
use log::info;
use svc_telemetry::shutdown_signal;

/// struct holding cli configuration options
#[derive(Parser, Debug)]
pub struct Cli {
    /// Target file to write the OpenAPI Spec
    #[arg(long)]
    pub openapi: Option<String>,
}

#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("(svc-telemetry) server startup.");

    // Check for expected environment variables
    let config = match config::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            println!("(config) could not generate config: {}.", e);
            panic!();
        }
    };

    // Start Logger
    let log_cfg: &str = config.log_config.as_str();
    if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
        println!("(logger) could not parse {}: {}.", log_cfg, e);
        panic!();
    }

    // Allow option to only generate the spec file to a given location
    // use `make rust-openapi` to generate the OpenAPI specification
    let args = Cli::parse();
    if let Some(target) = args.openapi {
        return rest::generate_openapi_spec(&target);
    }

    // REST Server
    tokio::spawn(rest::server::rest_server(config.clone()));

    // GRPC Server
    let _ = tokio::spawn(grpc::server::grpc_server(config)).await;

    info!("(svc-telemetry) server shutdown.");
    Ok(())
}
