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

/// Name of the AMQP exchange for telemetry messages
pub const EXCHANGE_NAME_TELEMETRY: &str = "telemetry";

/// Name of the AMQP queue for ADSB messages
pub const QUEUE_NAME_ADSB: &str = "adsb";

/// Routing key for ADSB messages
pub const ROUTING_KEY_ADSB: &str = "adsb";

/// Initializes the AMQP connection. Creates the telemetry exchange and queues.
pub async fn init_mq(config: crate::config::Config) -> Result<lapin::Channel, ()> {
    let mq_host = config.rabbitmq_nodename;
    let mq_addr = format!("amqp://{mq_host}:5672");

    // Establish connection to RabbitMQ node
    rest_info!("(init_mq) connecting to MQ server at {}...", mq_addr);
    let result = lapin::Connection::connect(&mq_addr, lapin::ConnectionProperties::default()).await;
    let mq_connection = match result {
        Ok(conn) => conn,
        Err(e) => {
            rest_error!("(init_mq) could not connect to MQ server at {mq_addr}.");
            rest_debug!("(init_mq) error: {:?}", e);
            return Err(());
        }
    };

    // Create channel
    rest_info!("(init_mq) creating channel at {}...", mq_addr);
    let mq_channel = match mq_connection.create_channel().await {
        Ok(channel) => channel,
        Err(e) => {
            rest_error!("(init_mq) could not create channel at {mq_addr}.");
            rest_debug!("(init_mq) error: {:?}", e);
            return Err(());
        }
    };

    // Declare ADSB Queue
    {
        rest_info!("(init_mq) creating '{QUEUE_NAME_ADSB}' queue...");
        let result = mq_channel
            .queue_declare(
                QUEUE_NAME_ADSB,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await;

        if let Err(e) = result {
            rest_error!("(init_mq) could not declare queue '{QUEUE_NAME_ADSB}'.");
            rest_debug!("(init_mq) error: {:?}", e);
            return Err(());
        }
    }

    //
    // Declare a topic exchange
    //
    {
        rest_info!("(init_mq) declaring exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        let result = mq_channel
            .exchange_declare(
                EXCHANGE_NAME_TELEMETRY,
                lapin::ExchangeKind::Topic,
                lapin::options::ExchangeDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await;

        if let Err(e) = result {
            rest_error!("(init_mq) could not declare exchange '{EXCHANGE_NAME_TELEMETRY}'.");
            rest_debug!("(init_mq) error: {:?}", e);
            return Err(());
        }
    }

    //
    // Bind the ADSB queue to the exchange
    //
    {
        rest_info!("(init_mq) binding queue '{QUEUE_NAME_ADSB}' to exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        let result = mq_channel
            .queue_bind(
                QUEUE_NAME_ADSB,
                EXCHANGE_NAME_TELEMETRY,
                ROUTING_KEY_ADSB,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await;

        if let Err(e) = result {
            rest_error!("(init_mq) could not bind queue '{QUEUE_NAME_ADSB}' to exchange.");
            rest_debug!("(init_mq) error: {:?}", e);
        }
    }

    // TODO(R4): Telemetry from other assets

    Ok(mq_channel)
}

/// Starts the REST API server for this microservice
#[cfg(not(tarpaulin_include))]
pub async fn rest_server(config: crate::config::Config) -> Result<(), ()> {
    rest_info!("(rest_server) entry.");
    let rest_port = config.docker_port_rest;

    //
    // Extensions
    //

    // GRPC Clients
    let grpc_clients = GrpcClients::default(config.clone());

    // Redis Caches
    let mavlink_cache =
        RedisPool::new(config.clone(), "tlm:mav", CACHE_EXPIRE_MS_MAVLINK_ADSB).await?;

    let adsb_cache =
        RedisPool::new(config.clone(), "tlm:adsb", CACHE_EXPIRE_MS_AIRCRAFT_ADSB).await?;

    // RabbitMQ Channel
    let mq_channel = init_mq(config.clone()).await?;

    //
    // Create Server
    //
    let app = Router::new()
        .route("/health", routing::get(api::health_check))
        .route("/telemetry/mavlink/adsb", routing::post(api::mavlink_adsb))
        .route("/telemetry/aircraft/adsb", routing::post(api::adsb))
        .layer(Extension(mavlink_cache))
        .layer(Extension(adsb_cache))
        .layer(Extension(mq_channel))
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
