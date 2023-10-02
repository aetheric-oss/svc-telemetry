//! Redis connection pool implementation

use core::fmt::{Debug, Formatter};
use deadpool_redis::{redis, Pool, Runtime};
use snafu::prelude::Snafu;

/// Represents a pool of connections to a Redis server.
///
/// The [`RedisPool`] struct provides a managed pool of connections to a Redis server.
/// It allows clients to acquire and release connections from the pool and handles
/// connection management, such as connection pooling and reusing connections.
#[derive(Clone)]
pub struct RedisPool {
    /// The underlying pool of Redis connections.
    pool: Pool,
    /// The string prepended to the key being stored.
    key_folder: String,
}
impl Debug for RedisPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RedisPool")
            .field("key_folder", &self.key_folder)
            .finish()
    }
}

/// Represents errors that can occur during cache operations.
#[derive(Debug, Clone, Copy, Snafu)]
pub enum CacheError {
    /// Could not build configuration for cache.
    #[snafu(display("Could not build configuration for cache."))]
    CouldNotConfigure,

    /// Could not connect to the Redis pool.
    #[snafu(display("Could not connect to redis pool."))]
    CouldNotConnect,

    /// The operation on the Redis cache failed.
    #[snafu(display("The operation on the redis cache failed."))]
    OperationFailed,
}

impl RedisPool {
    /// Create a new RedisPool
    /// The 'key_folder' argument is prepended to the key being stored. The
    ///  complete key will take the format \<folder\>:\<subset\>:\<subset\>:\<key\>.
    ///  This is used to differentiate keys inserted into Redis by different
    ///  microservices. For example, an ADS-B key in svc-telemetry might be
    ///  formatted `telemetry:adsb:1234567890`.
    pub async fn new(config: crate::config::Config, key_folder: &str) -> Result<Self, ()> {
        // the .env file must have REDIS__URL="redis://\<host\>:\<port\>"
        let cfg: deadpool_redis::Config = config.redis;
        let Some(details) = cfg.url.clone() else {
            cache_error!("(RedisPool new) no connection address found.");
            return Err(());
        };

        cache_info!(
            "(RedisPool new) creating pool with key folder '{}' at {:?}...",
            key_folder,
            details
        );
        match cfg.create_pool(Some(Runtime::Tokio1)) {
            Ok(pool) => {
                cache_info!("(RedisPool new) pool created.");
                Ok(RedisPool {
                    pool,
                    key_folder: String::from(key_folder),
                })
            }
            Err(e) => {
                cache_error!("(RedisPool new) could not create pool: {}", e);
                Err(())
            }
        }
    }

    /// If the key didn't exist, inserts the key with an expiration time.
    /// If the key exists, increments the key and doesn't extend the expiration time.
    ///
    /// Returns the order in which this specific key was received (1 for first time).
    pub async fn increment(&mut self, key: &str, expiration_ms: u32) -> Result<u32, CacheError> {
        let key = format!("{}:{}", &self.key_folder, key);
        cache_info!("(increment) entry with key {}.", &key);

        let mut connection = match self.pool.get().await {
            Ok(connection) => connection,
            Err(e) => {
                cache_error!("(increment) could not connect to redis deadpool: {e}");
                return Err(CacheError::CouldNotConnect);
            }
        };

        let mut result = match redis::pipe()
            .atomic()
            // Return the value of this increment (1 if key didn't exist before)
            .cmd("INCR")
            .arg(&[&key])
            // Set the expiration time
            .cmd("PEXPIRE")
            .arg(key)
            .arg(expiration_ms)
            // .arg("NX") // only if it didn't have one before
            // (not implemented in `redis` crate yet: https://redis.io/commands/pexpire/)
            .ignore()
            .query_async::<_, _>(&mut connection)
            .await
        {
            Ok(redis::Value::Bulk(val)) => val,
            Ok(value) => {
                cache_error!(
                    "(increment) Operation failed, unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                cache_error!("(increment) Operation failed, redis error: {}", e);
                return Err(CacheError::OperationFailed);
            }
        };

        let new_value = match result.pop() {
            Some(redis::Value::Int(new_value)) => new_value,
            Some(value) => {
                cache_error!(
                    "(increment) Operation failed, unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            None => {
                cache_error!("(increment) Operation failed, empty redis response array.");
                return Err(CacheError::OperationFailed);
            }
        };

        // Received value should be greater than 0, return a u32 type
        if new_value < 1 {
            cache_error!(
                "(increment) operation failed, unexpected value: {:?}",
                new_value
            );
            return Err(CacheError::OperationFailed);
        }

        Ok(new_value as u32)
    }

    ///
    /// Set the value of multiple keys
    ///
    pub async fn multiple_set(
        &mut self,
        keyvals: Vec<(String, String)>,
        expiration_ms: u32,
    ) -> Result<(), CacheError> {
        let mut connection = match self.pool.get().await {
            Ok(connection) => connection,
            Err(e) => {
                cache_error!("(multiple_set) could not connect to redis deadpool: {e}");
                return Err(CacheError::CouldNotConnect);
            }
        };

        let mut pipe = redis::pipe();
        let mut pipe_ref = pipe.atomic();
        for (key, value) in keyvals {
            // Set the expiration time
            pipe_ref = pipe_ref
                .pset_ex(key, value, expiration_ms as usize)
                .ignore();
        }

        match pipe.query_async(&mut connection).await {
            Ok(redis::Value::Okay) => Ok(()),
            Ok(value) => {
                cache_error!(
                    "(multiple_set) Operation failed, unexpected redis response: {:?}",
                    value
                );
                Err(CacheError::OperationFailed)
            }
            Err(e) => {
                cache_error!("(multiple_set) Operation failed, redis error: {}", e);
                Err(CacheError::OperationFailed)
            }
        }
    }

    ///
    /// Get the value of multiple keys
    ///
    pub async fn multiple_get<T: std::str::FromStr>(
        &mut self,
        keys: Vec<String>,
    ) -> Result<Vec<T>, CacheError> {
        let mut connection = match self.pool.get().await {
            Ok(connection) => connection,
            Err(e) => {
                cache_error!("(multiple_get) could not connect to redis deadpool: {e}");
                return Err(CacheError::CouldNotConnect);
            }
        };

        let result = redis::pipe()
            .atomic()
            .mget(keys.join(" "))
            .query_async(&mut connection)
            .await;

        match result {
            Ok(redis::Value::Bulk(values)) => {
                let values = values
                    .iter()
                    .filter_map(|value| match value {
                        redis::Value::Data(data) => {
                            let Ok(str) = String::from_utf8(data.to_vec()) else {
                                cache_error!(
                                    "(multiple_get) Operation failed, could not parse redis response."
                                );
                                return None;
                            };

                            T::from_str(&str).ok()
                        }
                        _ => None,
                    })
                    .collect::<Vec<T>>();

                if values.len() != keys.len() {
                    cache_error!(
                        "(multiple_get) Operation failed, expected {} values, got {}.",
                        keys.len(),
                        values.len()
                    );
                    return Err(CacheError::OperationFailed);
                }

                Ok(values)
            }
            Ok(value) => {
                cache_error!(
                    "(multiple_get) Operation failed, unexpected redis response: {:?}",
                    value
                );
                Err(CacheError::OperationFailed)
            }
            Err(e) => {
                cache_error!("(multiple_get) Operation failed, redis error: {}", e);
                Err(CacheError::OperationFailed)
            }
        }
    }
}
