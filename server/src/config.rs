//! # Config
//!
//! Define and implement config options for module

use anyhow::Result;
use config::{ConfigError, Environment};
use dotenv::dotenv;
use lapin::ConnectionProperties;
use serde::Deserialize;

/// struct holding configuration options
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// port to be used for gRPC server
    pub docker_port_grpc: u16,
    /// port to be used for REST server
    pub docker_port_rest: u16,
    /// host of storage server
    pub storage_host_grpc: String,
    /// port of storage server
    pub storage_port_grpc: u16,
    /// config to be used for the RabbitMQ connection
    pub amqp: deadpool_lapin::Config,
    /// config to be used for the Redis server
    pub redis: deadpool_redis::Config,
    /// path to log configuration YAML file
    pub log_config: String,
}

impl Default for Config {
    fn default() -> Self {
        log::warn!("Creating Config object with default values.");
        Self::new()
    }
}

impl Config {
    /// Default values for Config
    pub fn new() -> Self {
        Config {
            docker_port_grpc: 50051,
            docker_port_rest: 8000,
            storage_port_grpc: 50051,
            storage_host_grpc: "localhost".to_owned(),
            redis: deadpool_redis::Config {
                url: None,
                pool: None,
                connection: None,
            },
            amqp: deadpool_lapin::Config {
                url: None,
                pool: None,
                connection_properties: ConnectionProperties::default(),
            },
            log_config: String::from("log4rs.yaml"),
        }
    }

    /// Create a new `Config` object using environment variables
    pub fn try_from_env() -> Result<Self, ConfigError> {
        // read .env file if present
        dotenv().ok();

        config::Config::builder()
            .set_default("docker_port_grpc", 50051)?
            .set_default("docker_port_rest", 8000)?
            .set_default("log_config", String::from("log4rs.yaml"))?
            .add_source(Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_default() {
        let config = Config::default();

        assert_eq!(config.docker_port_grpc, 50051);
        assert_eq!(config.docker_port_rest, 8000);
        assert_eq!(config.storage_port_grpc, 50051);
        assert_eq!(config.storage_host_grpc, String::from("localhost"));
        assert!(config.amqp.url.is_none());
        assert!(config.amqp.pool.is_none());
        assert!(config.redis.url.is_none());
        assert!(config.redis.pool.is_none());
        assert!(config.redis.connection.is_none());
        assert_eq!(config.log_config, String::from("log4rs.yaml"));
    }
    #[test]
    fn test_config_from_env() {
        std::env::set_var("DOCKER_PORT_GRPC", "6789");
        std::env::set_var("DOCKER_PORT_REST", "9876");
        std::env::set_var("STORAGE_HOST_GRPC", "test_host_grpc");
        std::env::set_var("STORAGE_PORT_GRPC", "12345");
        std::env::set_var("AMQP__URL", "amqp://test_rabbitmq:5672");
        std::env::set_var("AMQP__POOL__MAX_SIZE", "16");
        std::env::set_var("AMQP__POOL__TIMEOUTS__WAIT__SECS", "2");
        std::env::set_var("AMQP__POOL__TIMEOUTS__WAIT__NANOS", "0");
        std::env::set_var("REDIS__URL", "redis://test_redis:6379");
        std::env::set_var("REDIS__POOL__MAX_SIZE", "16");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__SECS", "2");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__NANOS", "0");
        std::env::set_var("LOG_CONFIG", "config_file.yaml");
        let config = Config::try_from_env();
        assert!(config.is_ok());
        let config = config.unwrap();

        assert_eq!(config.docker_port_grpc, 6789);
        assert_eq!(config.storage_port_grpc, 12345);
        assert_eq!(config.storage_host_grpc, String::from("test_host_grpc"));
        assert_eq!(config.log_config, String::from("config_file.yaml"));
        assert_eq!(
            config.amqp.url,
            Some(String::from("amqp://test_rabbitmq:5672"))
        );
        assert!(config.amqp.pool.is_some());
        assert_eq!(
            config.redis.url,
            Some(String::from("redis://test_redis:6379"))
        );
        assert!(config.redis.pool.is_some());
    }
}
