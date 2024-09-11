//! provides AMQP/RabbitMQ implementations for queuing layer

#[macro_use]
pub mod macros;
pub mod pool;
use crate::config::Config;
use snafu::prelude::Snafu;

/// Name of the AMQP exchange for telemetry messages
pub const EXCHANGE_NAME_TELEMETRY: &str = "telemetry";

/// Name of the AMQP queue for ADSB messages
pub const QUEUE_NAME_ADSB: &str = "adsb";

/// Routing key for ADSB messages
pub const ROUTING_KEY_ADSB: &str = "adsb";

/// Name of the AMQP queue for NETRID identification messages
pub const QUEUE_NAME_NETRID_ID: &str = "netrid_id";

/// Name of the AMQP queue for NETRID position messages
pub const QUEUE_NAME_NETRID_POSITION: &str = "netrid_pos";

/// Name of the AMQP queue for NETRID velocity messages
pub const QUEUE_NAME_NETRID_VELOCITY: &str = "netrid_vel";

/// Routing key for NETRID Identification messages
pub const ROUTING_KEY_NETRID_ID: &str = "netrid:id";

/// Routing key for NETRID Position messages
pub const ROUTING_KEY_NETRID_POSITION: &str = "netrid:pos";

/// Routing key for NETRID Velocity messages
pub const ROUTING_KEY_NETRID_VELOCITY: &str = "netrid:vel";

/// Custom Error type for MQ errors
#[derive(Debug, Snafu, Clone, Copy, PartialEq)]
pub enum AMQPError {
    /// Could Not Publish
    #[snafu(display("Could not publish to queue."))]
    CouldNotPublish,

    /// Could not connect to the AMQP pool.
    #[snafu(display("Could not connect to amqp pool."))]
    CouldNotConnect,

    /// Missing configuration
    #[snafu(display("Missing configuration for amqp pool connection."))]
    MissingConfiguration,

    /// Could not create channel
    #[snafu(display("Could not create channel."))]
    CouldNotCreateChannel,

    /// Could not declare queue
    #[snafu(display("Could not declare queue."))]
    CouldNotDeclareQueue,

    /// Could not bind queue
    #[snafu(display("Could not bind queue."))]
    CouldNotBindQueue,

    /// Could not declare exchange
    #[snafu(display("Could not declare exchange."))]
    CouldNotDeclareExchange,
}

/// Initializes the AMQP connection. Creates the telemetry exchange and queues.
#[cfg(not(test))]
#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) need rabbitmq backend running, integration tests
pub async fn init_mq(config: Config) -> Result<lapin::Channel, AMQPError> {
    // Establish connection to RabbitMQ node
    let pool = pool::AMQPPool::new(config.clone())?;
    let amqp_connection = pool.get_connection().await?;

    //
    // Create channel
    //
    amqp_info!("creating channel...");
    let amqp_channel = amqp_connection.create_channel().await.map_err(|e| {
        amqp_error!("could not create channel.");
        amqp_debug!("error: {:?}", e);
        AMQPError::CouldNotCreateChannel
    })?;

    //
    // Declare a topic exchange
    //
    amqp_info!("declaring exchange '{EXCHANGE_NAME_TELEMETRY}'...");
    amqp_channel
        .exchange_declare(
            EXCHANGE_NAME_TELEMETRY,
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await
        .map_err(|e| {
            amqp_error!("could not declare exchange '{EXCHANGE_NAME_TELEMETRY}'.");
            amqp_debug!("error: {:?}", e);
            AMQPError::CouldNotDeclareExchange
        })?;

    //
    // Declare and Bind Queues
    //
    let queues = [
        (QUEUE_NAME_ADSB, ROUTING_KEY_ADSB),
        (QUEUE_NAME_NETRID_ID, ROUTING_KEY_NETRID_ID),
        (QUEUE_NAME_NETRID_POSITION, ROUTING_KEY_NETRID_POSITION),
        (QUEUE_NAME_NETRID_VELOCITY, ROUTING_KEY_NETRID_VELOCITY),
    ];

    for (queue, routing_key) in queues.iter() {
        amqp_info!("creating queue '{queue}'...");
        amqp_channel
            .queue_declare(
                queue,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("could not declare queue '{queue}'.");
                amqp_debug!("error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        amqp_info!("binding queue '{queue}' to exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        amqp_channel
            .queue_bind(
                queue,
                EXCHANGE_NAME_TELEMETRY,
                routing_key,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("could not bind queue '{queue}' to exchange.");
                amqp_debug!("error: {:?}", e);
                AMQPError::CouldNotBindQueue
            })?;
    }

    Ok(amqp_channel)
}

/// Initializes the AMQP connection. Creates the telemetry exchange and queues.
#[cfg(test)]
#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) this is a stub
pub async fn init_mq(_config: Config) -> Result<(), AMQPError> {
    Ok(())
}
