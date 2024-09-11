//! Redis connection pool implementation

use core::fmt::{Debug, Formatter};

#[cfg(not(test))]
use deadpool_redis::{redis, Pool, Runtime};

use serde::Serialize;
use snafu::prelude::Snafu;

/// Represents a pool of connections to a Redis server.
///
/// The [`TelemetryPool`] struct provides a managed pool of connections to a Redis server.
/// It allows clients to acquire and release connections from the pool and handles
/// connection management, such as connection pooling and reusing connections.
#[cfg(not(test))]
#[derive(Clone)]
pub struct TelemetryPool {
    /// The underlying pool of Redis connections.
    pool: Pool,
    /// The string prepended to the key being stored.
    key_folder: String,
}

/// Represents a pool of connections to a Redis server.
/// No pool in test environment.
#[derive(Clone)]
#[cfg(test)]
pub struct TelemetryPool {
    /// The string prepended to the key being stored.
    key_folder: String,
}

/// Represents a pool of connections to a Redis server for GIS-related data
#[derive(Clone)]
#[cfg(not(test))]
pub struct GisPool {
    /// The underlying pool of Redis connections.
    pool: Pool,
}

#[derive(Clone, Copy)]
#[cfg(test)]
pub struct GisPool {}

impl Debug for TelemetryPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TelemetryPool")
            .field("key_folder", &self.key_folder)
            .finish()
    }
}

impl Debug for GisPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GisPool").finish()
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

#[cfg(test)]
impl GisPool {
    /// Create a new GisPool
    pub async fn new(_config: crate::config::Config) -> Result<Self, ()> {
        println!("(MOCK) creating pool...");
        Ok(GisPool {})
    }

    /// Push items onto a redis queue
    pub async fn push<T>(&mut self, _item: T, _queue_key: &str) -> Result<(), ()>
    where
        T: Serialize + Debug,
    {
        println!("(MOCK) pushing...");
        Ok(())
    }
}

#[cfg(not(test))]
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need redis backend to test
impl GisPool {
    /// Create a new GisPool
    pub async fn new(config: crate::config::Config) -> Result<Self, ()> {
        let cfg: deadpool_redis::Config = config.redis;
        let details = cfg.url.clone().ok_or_else(|| {
            cache_error!("(GisPool new) no connection address found.");
        })?;

        cache_info!("(GisPool new) creating pool at {:?}...", details);

        let pool = cfg.create_pool(Some(Runtime::Tokio1)).map_err(|e| {
            cache_error!("(GisPool new) could not create pool: {}", e);
        })?;

        Ok(GisPool { pool })
    }

    /// Push items onto a redis queue
    pub async fn push<T>(&mut self, item: T, queue_key: &str) -> Result<(), ()>
    where
        T: Serialize + Debug,
    {
        if queue_key.is_empty() {
            cache_error!("queue key cannot be empty.");
            return Err(());
        }

        let serialized = serde_json::to_vec(&item).map_err(|e| {
            cache_error!("could not serialize item {:#?}: {e}", item);
        })?;

        let mut connection = self.pool.get().await.map_err(|e| {
            cache_error!("could not connect to redis deadpool: {e}");
        })?;

        let result = redis::pipe()
            .atomic()
            .lpush(queue_key, serialized)
            .query_async(&mut connection)
            .await
            .map_err(|e| {
                cache_error!("Operation failed, redis error: {}", e);
            })?;

        let redis::Value::Bulk(values) = result else {
            cache_error!("Operation failed, unexpected redis response: {:?}", result);

            return Err(());
        };

        match values.len() {
            1 => Ok(()),
            _ => {
                cache_error!("Operation failed, unexpected redis response: {:?}", values);
                Err(())
            }
        }
    }
}

#[cfg(not(test))]
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need redis backend to test
impl TelemetryPool {
    /// Create a new TelemetryPool
    /// The 'key_folder' argument is prepended to the key being stored. The
    ///  complete key will take the format \<folder\>:\<subset\>:\<subset\>:\<key\>.
    ///  This is used to differentiate keys inserted into Redis by different
    ///  microservices. For example, an ADS-B key in svc-telemetry might be
    ///  formatted `telemetry:adsb:1234567890`.
    pub async fn new(config: crate::config::Config, key_folder: &str) -> Result<Self, ()> {
        if key_folder.is_empty() {
            cache_error!("(TelemetryPool new) key folder cannot be empty.");
            return Err(());
        }

        // the .env file must have REDIS__URL="redis://\<host\>:\<port\>"
        let cfg: deadpool_redis::Config = config.redis;
        let details = cfg.url.clone().ok_or_else(|| {
            cache_error!("(TelemetryPool new) no connection address found.");
        })?;

        cache_info!(
            "(TelemetryPool new) creating pool with key folder '{}' at {:?}...",
            key_folder,
            details
        );

        let pool = cfg.create_pool(Some(Runtime::Tokio1)).map_err(|e| {
            cache_error!("(TelemetryPool new) could not create pool: {}", e);
        })?;

        cache_info!("(TelemetryPool new) pool created.");
        Ok(TelemetryPool {
            pool,
            key_folder: String::from(key_folder),
        })
    }

    /// If the key didn't exist, inserts the key with an expiration time.
    /// If the key exists, increments the key and doesn't extend the expiration time.
    ///
    /// Returns the order in which this specific key was received (1 for first time).
    pub async fn increment(&mut self, key: &str, expiration_ms: u32) -> Result<u32, CacheError> {
        let key = format!("{}:{}", &self.key_folder, key);
        cache_info!("entry with key {}.", &key);

        let mut connection = self.pool.get().await.map_err(|e| {
            cache_error!("could not connect to redis deadpool: {e}");
            CacheError::CouldNotConnect
        })?;

        let result = redis::pipe()
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
            .map_err(|e| {
                cache_error!("Operation failed, redis error: {}", e);
                CacheError::OperationFailed
            })?;

        let redis::Value::Bulk(mut values) = result else {
            cache_error!("Operation failed, unexpected redis response: {:?}", result);

            return Err(CacheError::OperationFailed);
        };

        let value = values.pop().ok_or_else(|| {
            cache_error!("Operation failed, empty redis response array.");
            CacheError::OperationFailed
        })?;

        let redis::Value::Int(value) = value else {
            cache_error!("Operation failed, unexpected redis response: {:?}", value);
            return Err(CacheError::OperationFailed);
        };

        // Received value should be greater than 0, return a u32 type
        if value < 1 {
            cache_error!("operation failed, unexpected value: {:?}", value);

            return Err(CacheError::OperationFailed);
        }

        Ok(value as u32)
    }

    ///
    /// Set the value of multiple keys
    ///
    pub async fn multiple_set(
        &mut self,
        keyvals: Vec<(String, String)>,
        expiration_ms: u32,
    ) -> Result<(), CacheError> {
        let mut connection = self.pool.get().await.map_err(|e| {
            cache_error!("could not connect to redis deadpool: {e}");
            CacheError::CouldNotConnect
        })?;

        let mut pipe = redis::pipe();
        let mut pipe_ref = pipe.atomic();
        for (key, value) in keyvals {
            // Set the expiration time
            pipe_ref = pipe_ref
                .pset_ex(key, value, expiration_ms as usize)
                .ignore();
        }

        let result = pipe.query_async(&mut connection).await.map_err(|e| {
            cache_error!("Operation failed, redis error: {}", e);
            CacheError::OperationFailed
        })?;

        match result {
            redis::Value::Okay => Ok(()),
            value => {
                cache_error!("Operation failed, unexpected redis response: {:?}", value);

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
        let mut connection = self.pool.get().await.map_err(|e| {
            cache_error!("could not connect to redis deadpool: {e}");
            CacheError::CouldNotConnect
        })?;

        let result = redis::pipe()
            .atomic()
            .mget(keys.join(" "))
            .query_async(&mut connection)
            .await
            .map_err(|e| {
                cache_error!("Operation failed, redis error: {}", e);
                CacheError::OperationFailed
            })?;

        let redis::Value::Bulk(values) = result else {
            cache_error!("Operation failed, unexpected redis response: {:?}", result);

            return Err(CacheError::OperationFailed);
        };

        let values = values
            .iter()
            .filter_map(|value| match value {
                redis::Value::Data(data) => {
                    let Ok(str) = String::from_utf8(data.to_vec()) else {
                        cache_error!("Operation failed, could not parse redis response.");
                        return None;
                    };

                    T::from_str(&str).ok()
                }
                _ => None,
            })
            .collect::<Vec<T>>();

        if values.len() != keys.len() {
            cache_error!(
                "Operation failed, expected {} values, got {}.",
                keys.len(),
                values.len()
            );

            return Err(CacheError::OperationFailed);
        }

        Ok(values)
    }
}

#[cfg(test)]
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need redis backend to test
impl TelemetryPool {
    /// Create a new TelemetryPool
    /// The 'key_folder' argument is prepended to the key being stored. The
    ///  complete key will take the format \<folder\>:\<subset\>:\<subset\>:\<key\>.
    ///  This is used to differentiate keys inserted into Redis by different
    ///  microservices. For example, an ADS-B key in svc-telemetry might be
    ///  formatted `telemetry:adsb:1234567890`.
    pub async fn new(_config: crate::config::Config, key_folder: &str) -> Result<Self, ()> {
        cache_info!("pool created.");
        Ok(TelemetryPool {
            key_folder: String::from(key_folder),
        })
    }

    /// If the key didn't exist, inserts the key with an expiration time.
    /// If the key exists, increments the key and doesn't extend the expiration time.
    ///
    /// Returns the order in which this specific key was received (1 for first time).
    pub async fn increment(&mut self, _key: &str, _expiration_ms: u32) -> Result<u32, CacheError> {
        Ok(1)
    }

    ///
    /// Set the value of multiple keys
    ///
    pub async fn multiple_set(
        &mut self,
        _keyvals: Vec<(String, String)>,
        _expiration_ms: u32,
    ) -> Result<(), CacheError> {
        Ok(())
    }

    ///
    /// Get the value of multiple keys
    ///
    pub async fn multiple_get<T: std::str::FromStr>(
        &mut self,
        _keys: Vec<String>,
    ) -> Result<Vec<T>, CacheError> {
        Ok(vec![])
    }
}
