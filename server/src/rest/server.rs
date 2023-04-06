//! Rest server implementation

use super::api;
use crate::cache::pool::RedisPool;
use crate::grpc::client::GrpcClients;
use crate::shutdown_signal;
use axum::{extract::Extension, routing, Router};

/// Mavlink entries in the cache will expire after 5 seconds
const CACHE_EXPIRE_MS_MAVLINK_ADSB: u32 = 5000;

/// Mavlink entries in the cache will expire after 10 seconds
const CACHE_EXPIRE_MS_AIRCRAFT_ADSB: u32 = 10000;

/// Starts the REST API server for this microservice
#[cfg(not(tarpaulin_include))]
pub async fn rest_server(config: crate::config::Config) -> Result<(), ()> {
    rest_info!("(rest_server) entry.");
    let rest_port = config.docker_port_rest;

    //
    // Extensions
    //
    let grpc_clients = GrpcClients::default();

    let mavlink_cache = RedisPool::new(config.clone(), CACHE_EXPIRE_MS_MAVLINK_ADSB)
        .await
        .expect("Could not start redis server.");

    let adsb_cache = RedisPool::new(config.clone(), CACHE_EXPIRE_MS_AIRCRAFT_ADSB)
        .await
        .expect("Could not start redis server.");

    //
    // Create Server
    //
    let app = Router::new()
        .route("/health", routing::get(api::health_check))
        .route("/telemetry/mavlink/adsb", routing::post(api::mavlink_adsb))
        .route("/telemetry/aircraft/adsb", routing::post(api::adsb))
        .layer(Extension(mavlink_cache))
        .layer(Extension(adsb_cache))
        .layer(Extension(grpc_clients));

    let address = format!("[::]:{rest_port}");
    let Ok(address) = address.parse() else {
        rest_error!("(rest_server) invalid address: {:?}, exiting.", address);
        return Err(());
    };

    //
    // Bind to address
    //
    rest_info!("(rest_server) hosted at {:?}.", address);
    let _ = axum::Server::bind(&address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal("rest"))
        .await;

    Ok(())
}
