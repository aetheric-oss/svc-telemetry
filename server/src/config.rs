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
    /// host of gis server
    pub gis_host_grpc: String,
    /// port of gis server
    pub gis_port_grpc: u16,
    /// config to be used for the RabbitMQ connection
    pub amqp: deadpool_lapin::Config,
    /// config to be used for the Redis server
    pub redis: deadpool_redis::Config,
    /// path to log configuration YAML file
    pub log_config: String,
    /// Ring buffer size
    pub ringbuffer_size_bytes: u16,
    /// Cadence for pushes to svc-gis
    pub gis_push_cadence_ms: u16,
    /// Maximum message size for gRPC message to svc-gis
    pub gis_max_message_size_bytes: u16,
    /// Rate limit - requests per second for REST requests
    pub rest_request_limit_per_second: u8,
    /// Enforces a limit on the concurrent number of requests the underlying service can handle
    pub rest_concurrency_limit_per_service: u8,
    /// Full url (including port number) to be allowed as request origin for
    /// REST requests
    pub rest_cors_allowed_origin: String,
}

impl Default for Config {
    fn default() -> Self {
        log::warn!("(Config Default) Creating Config object with default values.");
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
            gis_port_grpc: 50051,
            gis_host_grpc: "localhost".to_owned(),
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
            ringbuffer_size_bytes: 4096,
            gis_push_cadence_ms: 50,
            gis_max_message_size_bytes: 2048,
            rest_request_limit_per_second: 2,
            rest_concurrency_limit_per_service: 5,
            rest_cors_allowed_origin: String::from("http://localhost:3000"),
        }
    }

    /// Create a new `Config` object using environment variables
    pub fn try_from_env() -> Result<Self, ConfigError> {
        // read .env file if present
        dotenv().ok();
        let default_config = Config::default();

        config::Config::builder()
            .set_default("docker_port_grpc", default_config.docker_port_grpc)?
            .set_default("docker_port_rest", default_config.docker_port_rest)?
            .set_default("log_config", default_config.log_config)?
            .set_default(
                "rest_concurrency_limit_per_service",
                default_config.rest_concurrency_limit_per_service,
            )?
            .set_default(
                "rest_request_limit_per_seconds",
                default_config.rest_request_limit_per_second,
            )?
            .set_default(
                "rest_cors_allowed_origin",
                default_config.rest_cors_allowed_origin,
            )?
            .set_default(
                "ringbuffer_size_bytes",
                default_config.ringbuffer_size_bytes,
            )?
            .set_default("gis_push_cadence_ms", default_config.gis_push_cadence_ms)?
            .set_default(
                "gis_max_message_size_bytes",
                default_config.gis_max_message_size_bytes,
            )?
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
        assert_eq!(config.gis_port_grpc, 50051);
        assert_eq!(config.gis_host_grpc, String::from("localhost"));
        assert!(config.amqp.url.is_none());
        assert!(config.amqp.pool.is_none());
        assert!(config.redis.url.is_none());
        assert!(config.redis.pool.is_none());
        assert!(config.redis.connection.is_none());
        assert_eq!(config.log_config, String::from("log4rs.yaml"));
        assert_eq!(config.ringbuffer_size_bytes, 4096);
        assert_eq!(config.gis_push_cadence_ms, 50);
        assert_eq!(config.gis_max_message_size_bytes, 2048);
        assert_eq!(config.rest_concurrency_limit_per_service, 5);
        assert_eq!(config.rest_request_limit_per_second, 2);
        assert_eq!(
            config.rest_cors_allowed_origin,
            String::from("http://localhost:3000")
        );
    }
    #[test]
    fn test_config_from_env() {
        std::env::set_var("DOCKER_PORT_GRPC", "6789");
        std::env::set_var("DOCKER_PORT_REST", "9876");
        std::env::set_var("STORAGE_HOST_GRPC", "test_host_grpc");
        std::env::set_var("STORAGE_PORT_GRPC", "12345");
        std::env::set_var("GIS_HOST_GRPC", "test_host_grpc");
        std::env::set_var("GIS_PORT_GRPC", "12345");
        std::env::set_var("AMQP__URL", "amqp://test_rabbitmq:5672");
        std::env::set_var("AMQP__POOL__MAX_SIZE", "16");
        std::env::set_var("AMQP__POOL__TIMEOUTS__WAIT__SECS", "2");
        std::env::set_var("AMQP__POOL__TIMEOUTS__WAIT__NANOS", "0");
        std::env::set_var("REDIS__URL", "redis://test_redis:6379");
        std::env::set_var("REDIS__POOL__MAX_SIZE", "16");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__SECS", "2");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__NANOS", "0");
        std::env::set_var("LOG_CONFIG", "config_file.yaml");
        std::env::set_var("RINGBUFFER_SIZE_BYTES", "4096");
        std::env::set_var("GIS_PUSH_CADENCE_MS", "255");
        std::env::set_var("GIS_MAX_MESSAGE_SIZE_BYTES", "255");
        std::env::set_var("REST_CONCURRENCY_LIMIT_PER_SERVICE", "255");
        std::env::set_var("REST_REQUEST_LIMIT_PER_SECOND", "255");
        std::env::set_var(
            "REST_CORS_ALLOWED_ORIGIN",
            "https://allowed.origin.host:443",
        );
        let config = Config::try_from_env();
        assert!(config.is_ok());
        let config = config.unwrap();

        assert_eq!(config.docker_port_grpc, 6789);
        assert_eq!(config.storage_port_grpc, 12345);
        assert_eq!(config.storage_host_grpc, String::from("test_host_grpc"));
        assert_eq!(config.gis_port_grpc, 12345);
        assert_eq!(config.gis_host_grpc, String::from("test_host_grpc"));
        assert_eq!(config.log_config, String::from("config_file.yaml"));
        assert_eq!(config.ringbuffer_size_bytes, 4096);
        assert_eq!(config.gis_push_cadence_ms, 255);
        assert_eq!(config.gis_max_message_size_bytes, 255);
        assert_eq!(config.rest_concurrency_limit_per_service, 255);
        assert_eq!(config.rest_request_limit_per_second, 255);
        assert_eq!(
            config.rest_cors_allowed_origin,
            String::from("https://allowed.origin.host:443")
        );
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
