//! Rest server implementation

use super::api;
use crate::amqp::init_mq;
use crate::cache::pool::RedisPool;
use crate::cache::RedisPools;
use crate::grpc::client::GrpcClients;
use crate::shutdown_signal;
use crate::Config;
use axum::{
    error_handling::HandleErrorLayer,
    extract::Extension,
    http::{HeaderValue, StatusCode},
    routing, BoxError, Router,
};
use std::net::SocketAddr;
use tower::{
    buffer::BufferLayer,
    limit::{ConcurrencyLimitLayer, RateLimitLayer},
    ServiceBuilder,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Mavlink entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_MAVLINK_ADSB: u32 = 5000;

/// Mavlink entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_AIRCRAFT_ADSB: u32 = 10000;

/// Starts the REST API server for this microservice
///
/// # Example:
/// ```
/// use svc_telemetry::rest::server::rest_server;
/// use svc_telemetry::Config;
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::default();
///     tokio::spawn(rest_server(config, None)).await;
///     Ok(())
/// }
/// ```
#[cfg(not(tarpaulin_include))]
// no_coverage: Needs running backends to work.
// Will be tested in integration tests.
pub async fn rest_server(
    config: Config,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<(), ()> {
    rest_info!("(rest_server) entry.");
    let rest_port = config.docker_port_rest;
    let full_rest_addr: SocketAddr = match format!("[::]:{}", rest_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            rest_error!("(rest_server) invalid address: {:?}, exiting.", e);
            return Err(());
        }
    };

    let cors_allowed_origin = match config.rest_cors_allowed_origin.parse::<HeaderValue>() {
        Ok(url) => url,
        Err(e) => {
            rest_error!(
                "(rest_server) invalid cors_allowed_origin address: {:?}, exiting.",
                e
            );
            return Err(());
        }
    };

    // Rate limiting
    let rate_limit = config.rest_request_limit_per_second as u64;
    let concurrency_limit = config.rest_concurrency_limit_per_service as usize;
    let limit_middleware = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(HandleErrorLayer::new(|e: BoxError| async move {
            rest_warn!("(server) too many requests: {}", e);
            (
                StatusCode::TOO_MANY_REQUESTS,
                "(server) too many requests.".to_string(),
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
    // GRPC Clients
    let grpc_clients = GrpcClients::default(config.clone());

    // Redis Pools
    let pools = RedisPools {
        mavlink: RedisPool::new(config.clone(), "tlm:mav", CACHE_EXPIRE_MS_MAVLINK_ADSB).await?,
        adsb: RedisPool::new(config.clone(), "tlm:adsb", CACHE_EXPIRE_MS_AIRCRAFT_ADSB).await?,
    };

    // RabbitMQ Channel
    let mq_channel = init_mq(config.clone())
        .await
        .map_err(|_| rest_error!("(rest_server) could not create RabbitMQ Channel."));

    //
    // Create Server
    //
    let app = Router::new()
        .route("/health", routing::get(api::health_check))
        .route("/telemetry/mavlink/adsb", routing::post(api::mavlink_adsb))
        .route("/telemetry/aircraft/adsb", routing::post(api::adsb))
        .layer(
            CorsLayer::new()
                .allow_origin(cors_allowed_origin)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .layer(limit_middleware)
        .layer(Extension(pools))
        .layer(Extension(mq_channel))
        .layer(Extension(grpc_clients));

    //
    // Bind to address
    //
    match axum::Server::bind(&full_rest_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal("rest", shutdown_rx))
        .await
    {
        Ok(_) => {
            rest_info!("(rest_server) hosted at: {}.", full_rest_addr);
            Ok(())
        }
        Err(e) => {
            rest_error!("(rest_server) could not start server: {}", e);
            Err(())
        }
    }
}
