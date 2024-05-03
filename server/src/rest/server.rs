//! Rest server implementation

use super::api;
use crate::amqp::init_mq;
use crate::cache::pool::{GisPool, TelemetryPool};
use crate::cache::TelemetryPools;
use crate::grpc::client::GrpcClients;
use crate::shutdown_signal;
use crate::Config;
use axum::{
    error_handling::HandleErrorLayer,
    extract::Extension,
    http::{HeaderValue, StatusCode},
    routing::{get, post},
    BoxError, Router,
};
use rand::{distributions::Alphanumeric, Rng};
use std::net::SocketAddr;
use tower::{
    buffer::BufferLayer,
    limit::{ConcurrencyLimitLayer, RateLimitLayer},
    ServiceBuilder,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Starts the REST API server for this microservice
///
/// # Example:
/// ```
/// use svc_telemetry::rest::server::rest_server;
/// use svc_telemetry::grpc::client::GrpcClients;
/// use svc_gis_client_grpc::prelude::types::{AircraftPosition, AircraftVelocity, AircraftId};
/// use svc_telemetry::Config;
/// use std::collections::VecDeque;
/// use std::sync::{Arc, Mutex};
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::default();
///     let grpc_clients = GrpcClients::default(config.clone());
///     tokio::spawn(rest_server(config, grpc_clients, None)).await;
///     Ok(())
/// }
/// ```
pub async fn rest_server(
    config: Config,
    grpc_clients: GrpcClients,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<(), ()> {
    rest_info!("entry.");
    let rest_port = config.docker_port_rest;
    let full_rest_addr: SocketAddr = match format!("[::]:{}", rest_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            rest_error!("invalid address: {:?}, exiting.", e);
            return Err(());
        }
    };

    let cors_allowed_origin = match config.rest_cors_allowed_origin.parse::<HeaderValue>() {
        Ok(url) => url,
        Err(e) => {
            rest_error!("invalid cors_allowed_origin address: {:?}, exiting.", e);
            return Err(());
        }
    };

    // Rate limiting
    let rate_limit = config.rest_request_limit_per_second as u64;
    let concurrency_limit = config.rest_concurrency_limit_per_service as usize;
    let limit_middleware = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(HandleErrorLayer::new(|e: BoxError| async move {
            rest_warn!("too many requests: {}", e);
            (
                StatusCode::TOO_MANY_REQUESTS,
                "(rest_server) too many requests.".to_string(),
            )
        }))
        .layer(BufferLayer::new(100))
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .layer(RateLimitLayer::new(
            rate_limit,
            std::time::Duration::from_secs(1),
        ));

    //
    // Extensions
    //

    // Redis Pools
    let tlm_pools = TelemetryPools {
        adsb: TelemetryPool::new(config.clone(), "tlm:adsb").await?,
        netrid: TelemetryPool::new(config.clone(), "tlm:netrid").await?,
    };

    let gis_pool = GisPool::new(config.clone()).await?;

    // RabbitMQ Channel
    let mq_channel = init_mq(config.clone()).await.map_err(|e| {
        rest_error!("could not create RabbitMQ Channel: {e}");
    })?;

    // TODO(R5): Replace with PKI certificates
    // Temporarily set JWT token to a random string
    match crate::rest::api::jwt::JWT_SECRET.set(
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(42)
            .map(char::from)
            .collect(),
    ) {
        Err(e) => {
            rest_error!("could not set JWT_SECRET: {}", e);
            return Err(());
        }
        _ => {
            rest_info!("set JWT_SECRET.");
        }
    }

    //
    // Create Server
    //
    let app = Router::new()
        // must be first with its route layer
        .route("/telemetry/netrid", post(api::netrid::network_remote_id))
        .route_layer(axum::middleware::from_fn(crate::rest::api::jwt::auth))
        // other routes after route_layer not affected
        .route("/health", get(api::health::health_check))
        .route("/telemetry/login", get(crate::rest::api::jwt::login))
        .route("/telemetry/adsb", post(api::adsb::adsb))
        .layer(
            CorsLayer::new()
                .allow_origin(cors_allowed_origin)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .layer(limit_middleware)
        .layer(Extension(tlm_pools))
        .layer(Extension(gis_pool))
        .layer(Extension(mq_channel))
        .layer(Extension(grpc_clients));

    match axum::Server::bind(&full_rest_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal("rest", shutdown_rx))
        .await
    {
        Ok(_) => {
            rest_info!("hosted at: {}.", full_rest_addr);
            Ok(())
        }
        Err(e) => {
            rest_error!("could not start server: {}", e);
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_start_and_shutdown() {
        use tokio::time::{sleep, Duration};
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        let config = Config::default();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Start the rest server
        tokio::spawn(rest_server(config, Some(shutdown_rx)));

        // Give the server time to get through the startup sequence (and thus code)
        sleep(Duration::from_secs(1)).await;

        // Shut down server
        assert!(shutdown_tx.send(()).is_ok());

        ut_info!("success");
    }
}
