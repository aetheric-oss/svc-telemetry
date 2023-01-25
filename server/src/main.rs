//! <center>
//! <img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />
//! </center>
//! <div align="center">
//!     <a href="https://github.com/Arrow-air/svc-telemetry/releases">
//!         <img src="https://img.shields.io/github/v/release/Arrow-air/svc-telemetry?include_prereleases" alt="GitHub release (latest by date including pre-releases)">
//!     </a>
//!     <a href="https://github.com/Arrow-air/svc-telemetry/tree/main">
//!         <img src="https://github.com/arrow-air/svc-telemetry/actions/workflows/rust_ci.yml/badge.svg?branch=main" alt="Rust Checks">
//!     </a>
//!     <a href="https://discord.com/invite/arrow">
//!         <img src="https://img.shields.io/discord/853833144037277726?style=plastic" alt="Arrow DAO Discord">
//!     </a>
//!     <br><br>
//! </div>
//!
//! `svc-telemetry` exposes a REST API for networked assets to post telemetry.
//!  It also broadcasts aircraft and vertiport location data.

#[allow(dead_code)]
mod grpc_clients;

///module generated from grpc.proto
pub mod grpc {
    #![allow(unused_qualifications, missing_docs)]
    include!("grpc.rs");
}

mod rest_api;

use axum::{extract::Extension, handler::Handler, response::IntoResponse, routing, Router};
use clap::Parser;
use grpc::svc_telemetry_rpc_server::{SvcTelemetryRpc, SvcTelemetryRpcServer};
use grpc::{QueryIsReady, ReadyResponse};
use grpc_clients::GrpcClients;
use log::{info, warn};
use tonic::{transport::Server, Request, Response, Status};
use utoipa::OpenApi;

#[derive(Parser, Debug)]
struct Cli {
    /// Target file to write the OpenAPI Spec
    #[arg(long)]
    openapi: Option<String>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        rest_api::adsb,
    ),
    components(
        schemas(
            rest_api::rest_types::AdsbPacket
        )
    ),
    tags(
        (name = "svc-telemetry", description = "svc-telemetry REST API")
    )
)]
struct ApiDoc;

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct SvcTelemetryImpl {}

#[tonic::async_trait]
impl SvcTelemetryRpc for SvcTelemetryImpl {
    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        _request: Request<QueryIsReady>,
    ) -> Result<Response<ReadyResponse>, Status> {
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}

/// Responds a NOT_FOUND status and error string
///
/// # Arguments
///
/// # Examples
///
/// ```
/// let app = Router::new()
///         .fallback(not_found.into_service());
/// ```
async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (
        axum::http::StatusCode::NOT_FOUND,
        format!("No route {}", uri),
    )
}

/// Tokio signal handler that will wait for a user to press CTRL+C.
/// We use this in our hyper `Server` method `with_graceful_shutdown`.
///
/// # Arguments
///
/// # Examples
///
/// ```
/// Server::bind(&"0.0.0.0:8000".parse().unwrap())
/// .serve(app.into_make_service())
/// .with_graceful_shutdown(shutdown_signal())
/// .await
/// .unwrap();
/// ```
async fn shutdown_signal(server: &str) {
    tokio::signal::ctrl_c()
        .await
        .expect("expect tokio signal ctrl-c");
    warn!("({}) shutdown signal", server);
}

/// Starts the grpc server for this microservice
async fn grpc_server() {
    // GRPC Server
    let grpc_port = std::env::var("DOCKER_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);

    let full_grpc_addr = format!("[::]:{}", grpc_port).parse().unwrap();
    let imp = SvcTelemetryImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<SvcTelemetryRpcServer<SvcTelemetryImpl>>()
        .await;

    //start server
    info!("(grpc) hosted at {}", full_grpc_addr);
    Server::builder()
        .add_service(health_service)
        .add_service(SvcTelemetryRpcServer::new(imp))
        .serve(full_grpc_addr)
        .await
        .unwrap();
}

/// Starts the REST API server for this microservice
pub async fn rest_server(grpc_clients: GrpcClients) {
    let rest_port = std::env::var("DOCKER_PORT_REST")
        .unwrap_or_else(|_| "8000".to_string())
        .parse::<u16>()
        .unwrap_or(8000);

    let app = Router::new()
        .fallback(not_found.into_service())
        .route("/telemetry/adsb", routing::post(rest_api::adsb))
        // .route("/cargo/query", routing::post(rest_api::query_flight))
        // .route("/cargo/confirm", routing::put(rest_api::confirm_flight))
        // .route(
        //     "/cargo/vertiports",
        //     routing::post(rest_api::query_vertiports),
        // )
        .layer(Extension(grpc_clients)); // Extension layer must be last

    let address = format!("[::]:{rest_port}").parse().unwrap();
    info!("(rest) hosted at {:?}", address);
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal("rest"))
        .await
        .unwrap();
}

/// Create OpenAPI3 Specification File
fn generate_openapi_spec(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = ApiDoc::openapi()
        .to_pretty_json()
        .expect("(ERROR) unable to write openapi specification to json.");

    std::fs::write(target, output).expect("(ERROR) unable to write json string to file.");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Allow option to only generate the spec file to a given location
    let args = Cli::parse();
    if let Some(target) = args.openapi {
        return generate_openapi_spec(&target);
    }

    // Start Logger
    let log_cfg: &str = "log4rs.yaml";
    if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
        println!("(logger) could not parse {}. {}", log_cfg, e);
        panic!();
    }

    // Start GRPC Server
    tokio::spawn(grpc_server());

    // Wait for other GRPC Servers
    let grpc_clients = GrpcClients::default();

    // Start REST API
    rest_server(grpc_clients).await;

    info!("Successful shutdown.");
    Ok(())
}
