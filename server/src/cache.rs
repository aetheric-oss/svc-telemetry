use deadpool_redis::{redis, Config, Pool, Runtime};
use snafu::prelude::Snafu;

const REDIS_POOL_SIZE: usize = 100;
const ENV_HOST: &str = "REDIS_HOST";
const ENV_PORT: &str = "REDIS_PORT";

#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct RedisPool {
    pool: Pool, // doesn't implement Debug
    expiration_ms: u32,
}

#[derive(Debug, Clone, Copy, Snafu)]
pub enum CacheError {
    #[snafu(display("Could not build configuration for cache."))]
    CouldNotConfigure,

    #[snafu(display("Could not connect to redis pool."))]
    CouldNotConnect,

    #[snafu(display("The operation on the redis cache failed."))]
    OperationFailed,

    #[snafu(display("The {ENV_HOST} env variable was not set."))]
    UndefinedHost,

    #[snafu(display("The {ENV_PORT} env variable was not set."))]
    UndefinedPort,
}

/// Writes an error! message to the app::cache logger
macro_rules! cache_error {
    ($($arg:tt)+) => {
        log::error!(target: "app::cache", $($arg)+);
    };
}

/// Writes a debug! message to the app::cache logger
macro_rules! cache_debug {
    ($($arg:tt)+) => {
        log::debug!(target: "app::cache", $($arg)+);
    };
}

impl RedisPool {
    pub async fn new(expiration_ms: u32) -> Result<Self, CacheError> {
        cache_debug!("(new) entry");
        let Ok(port) = std::env::var(ENV_PORT) else {
            cache_error!("(env) {} undefined.", ENV_PORT);
            return Err(CacheError::UndefinedPort);
        };

        let Ok(host) = std::env::var(ENV_HOST) else {
            cache_error!("(env) {} undefined.", ENV_HOST);
            return Err(CacheError::UndefinedHost);
        };

        let cfg = Config::from_url(format!("redis://{host}:{port}"));

        let Ok(builder) = cfg.builder() else {
            return Err(CacheError::CouldNotConfigure);
        };

        let Ok(pool) = builder
            .max_size(REDIS_POOL_SIZE)
            .runtime(Runtime::Tokio1)
            .build()
        else {
            return Err(CacheError::CouldNotConfigure);
        };

        Ok(RedisPool {
            pool,
            expiration_ms,
        })
    }

    /// If the key didn't exist, inserts the key with an expiration time.
    /// If the key exists, increments the key and doesn't extend the expiration time.
    ///
    /// # Returns
    /// The order in which this specific key was received (1 for first time).
    pub async fn try_key(&mut self, key: u32) -> Result<i64, CacheError> {
        cache_debug!("(try_key) entry with key {}", key);

        let Ok(mut connection) = self.pool.get().await else {
            cache_error!("(try_key) could not connect to redis deadpool.");
            return Err(CacheError::CouldNotConnect);
        };

        let return_values = redis::pipe()
            .atomic()
            // Return the value of this increment (1 if key didn't exist before)
            .cmd("INCR")
            .arg(&[key])
            // Set the expiration time
            .cmd("PEXPIRE")
            .arg(key)
            .arg(self.expiration_ms)
            // .arg("NX") // only if it didn't have one before
            // (not implemented in `redis` crate yet: https://redis.io/commands/pexpire/)
            .ignore()
            .query_async::<_, _>(&mut connection)
            .await;

        let Ok(redis::Value::Bulk(mut return_values)) = return_values else {
            cache_error!("(try_key) Operation failed.");
            cache_debug!("(try_key) Operation returned: {:?}", return_values.unwrap_err());
            return Err(CacheError::OperationFailed);
        };

        let Some(redis::Value::Int(new_value)) = return_values.pop() else {
            cache_error!("(try_key) Operation failed, empty redis response array.");
            return Err(CacheError::OperationFailed);
        };

        Ok(new_value)
    }
}
