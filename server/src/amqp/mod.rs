//! provides AMQP/RabbitMQ implementations for queuing layer

#[macro_use]
pub mod macros;
pub mod pool;
use crate::config::Config;
use lapin::{options::BasicPublishOptions, BasicProperties};
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
#[derive(Debug, Snafu, Clone, Copy)]
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

    /// Could not declare exchange
    #[snafu(display("Could not declare exchange."))]
    CouldNotDeclareExchange,
}

/// Wrapper struct to allow unit testing on un-connected amqp_channel
#[derive(Debug)]
pub struct AMQPChannel {
    /// The lapin::Channel if available
    pub channel: Option<lapin::Channel>,
}

cfg_if::cfg_if! {
    if #[cfg(feature = "test_util")] {
        impl AMQPChannel {
            /// Wrapper function for lapin::Channel basic_publish
            pub async fn basic_publish(
                &self,
                exchange: &str,
                routing_key: &str,
                options: BasicPublishOptions,
                payload: &[u8],
                properties: BasicProperties,
            ) -> Result<(), AMQPError> {
                if let Some(channel) = &self.channel {
                    match channel
                        .basic_publish(exchange, routing_key, options, payload, properties)
                        .await
                    {
                        Ok(_) => Ok(()),
                        Err(_) => Err(AMQPError::CouldNotPublish)
                    }
                } else {
                    Ok(())
                }
            }
        }
    } else {
        use lapin::publisher_confirm::PublisherConfirm;
        impl AMQPChannel {
            /// Wrapper function for lapin::Channel basic_publish
            pub async fn basic_publish(&self, exchange: &str, routing_key: &str, options: BasicPublishOptions, payload: &[u8], properties: BasicProperties) -> lapin::Result<PublisherConfirm> {
                if let Some(channel) = &self.channel {
                    channel.basic_publish(exchange, routing_key, options, payload, properties).await
                } else {
                    amqp_error!("(basic_publish) no channel set AMQPChannel.");
                    Err(lapin::Error::InvalidChannelState(lapin::ChannelState::Error))
                }
            }
        }
    }
}

/// Initializes the AMQP connection. Creates the telemetry exchange and queues.
#[cfg(not(tarpaulin_include))]
pub async fn init_mq(config: Config) -> Result<lapin::Channel, AMQPError> {
    // Establish connection to RabbitMQ node
    let pool = pool::AMQPPool::new(config.clone())?;

    let amqp_connection = pool.get_connection().await?;

    // Create channel
    amqp_info!("(init_mq) creating channel...");
    let amqp_channel = match amqp_connection.create_channel().await {
        Ok(channel) => channel,
        Err(e) => {
            amqp_error!("(init_mq) could not create channel.");
            amqp_debug!("(init_mq) error: {:?}", e);
            return Err(AMQPError::CouldNotCreateChannel);
        }
    };

    // Declare ADSB Queue
    {
        amqp_info!("(init_mq) creating queues...");
        let _ = amqp_channel
            .queue_declare(
                QUEUE_NAME_ADSB,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not declare queue '{QUEUE_NAME_ADSB}'.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        let _ = amqp_channel
            .queue_declare(
                QUEUE_NAME_NETRID_ID,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not declare queue '{QUEUE_NAME_NETRID_ID}'.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        let _ = amqp_channel
            .queue_declare(
                QUEUE_NAME_NETRID_POSITION,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not declare queue '{QUEUE_NAME_NETRID_POSITION}'.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        let _ = amqp_channel
            .queue_declare(
                QUEUE_NAME_NETRID_VELOCITY,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not declare queue '{QUEUE_NAME_NETRID_VELOCITY}'.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;
    }

    //
    // Declare a topic exchange
    //
    {
        amqp_info!("(init_mq) declaring exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        amqp_channel
            .exchange_declare(
                EXCHANGE_NAME_TELEMETRY,
                lapin::ExchangeKind::Topic,
                lapin::options::ExchangeDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not declare exchange '{EXCHANGE_NAME_TELEMETRY}'.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareExchange
            })?;
    }

    //
    // Bind the ADSB queue to the exchange
    //
    {
        amqp_info!("(init_mq) binding queue '{QUEUE_NAME_ADSB}' to exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        amqp_channel
            .queue_bind(
                QUEUE_NAME_ADSB,
                EXCHANGE_NAME_TELEMETRY,
                ROUTING_KEY_ADSB,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not bind queue '{QUEUE_NAME_ADSB}' to exchange.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        amqp_info!("(init_mq) binding queue '{QUEUE_NAME_NETRID_ID}' to exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        amqp_channel
            .queue_bind(
                QUEUE_NAME_NETRID_ID,
                EXCHANGE_NAME_TELEMETRY,
                ROUTING_KEY_NETRID_ID,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!("(init_mq) could not bind queue '{QUEUE_NAME_NETRID_ID}' to exchange.");
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        amqp_info!("(init_mq) binding queue '{QUEUE_NAME_NETRID_POSITION}' to exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        amqp_channel
            .queue_bind(
                QUEUE_NAME_NETRID_POSITION,
                EXCHANGE_NAME_TELEMETRY,
                ROUTING_KEY_NETRID_POSITION,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!(
                    "(init_mq) could not bind queue '{QUEUE_NAME_NETRID_POSITION}' to exchange."
                );
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;

        amqp_info!("(init_mq) binding queue '{QUEUE_NAME_NETRID_VELOCITY}' to exchange '{EXCHANGE_NAME_TELEMETRY}'...");
        amqp_channel
            .queue_bind(
                QUEUE_NAME_NETRID_VELOCITY,
                EXCHANGE_NAME_TELEMETRY,
                ROUTING_KEY_NETRID_VELOCITY,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                amqp_error!(
                    "(init_mq) could not bind queue '{QUEUE_NAME_NETRID_VELOCITY}' to exchange."
                );
                amqp_debug!("(init_mq) error: {:?}", e);
                AMQPError::CouldNotDeclareQueue
            })?;
    }

    Ok(amqp_channel)
}
