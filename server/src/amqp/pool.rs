//! AMQP connection pool implementation

use super::AMQPError;
use deadpool_lapin::{Object, Pool, Runtime};

/// Represents a pool of connections to a amqp server
///
/// The [`AMQPPool`] struct provides a managed pool of connections to a amqp/rabbitmq server.
/// It allows clients to acquire and release connections from the pool and handles
/// connection management, such as connection pooling and reusing connections.
#[derive(Clone, Debug)]
pub struct AMQPPool {
    /// The underlying pool of AMQP connections.
    pool: Pool,
}

impl AMQPPool {
    /// Create a new AMQP pool
    pub fn new(config: crate::config::Config) -> Result<Self, AMQPError> {
        // the .env file must have REDIS__URL="redis://<host>>:<port>"
        let cfg: deadpool_lapin::Config = config.amqp.clone();
        let details = cfg.url.clone().ok_or_else(|| {
            amqp_error!("(AMQPPool new) no connection address found.");
            amqp_debug!("(AMQPPool new) Available config: {:?}", &config.amqp);
            AMQPError::MissingConfiguration
        })?;

        amqp_info!("(AMQPPool new) creating pool at {:?}...", details);
        // no_coverage: this won't fail
        let pool: Pool = cfg.create_pool(Some(Runtime::Tokio1)).map_err(|e| {
            amqp_error!("(AMQPPool new) could not create pool: {}", e);
            AMQPError::CouldNotConnect
        })?;

        Ok(Self { pool })
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<Object, AMQPError> {
        self.pool.get().await.map_err(|e| {
            amqp_error!(
                "(AMQPPool get_connection) could not connect to deadpool: {}",
                e
            );
            AMQPError::CouldNotConnect
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_amqp_pool_new_failure() {
        let mut config = crate::config::Config::default();
        let result = AMQPPool::new(config.clone()).unwrap_err();
        assert_eq!(result, AMQPError::MissingConfiguration);

        // Invalid URL
        // config.amqp.url = Some("".to_string());
        // let result = AMQPPool::new(config.clone()).unwrap_err();
        // assert_eq!(result, AMQPError::CouldNotConnect);

        // Valid URL
        config.amqp.url = Some("amqp://localhost:5672".to_string());
        AMQPPool::new(config.clone()).unwrap();
    }

    #[tokio::test]
    #[cfg(not(feature = "stub_backends"))]
    async fn test_amqp_pool_new() {
        let config = crate::config::Config::default();
        let pool = AMQPPool::new(config.clone()).unwrap();
        let _ = pool.get_connection().await.unwrap();
    }
}
