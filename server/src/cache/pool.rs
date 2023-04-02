use deadpool_redis::{redis, Pool, Runtime};
use snafu::prelude::Snafu;

const REDIS_POOL_SIZE: usize = 100;

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
}

impl RedisPool {
    pub async fn new(
        config: crate::config::Config,
        expiration_ms: u32,
    ) -> Result<Self, CacheError> {
        cache_info!("(RedisPool new) entry.");
        let port = config.docker_port_redis;
        let host = config.docker_host_redis;

        let cfg = deadpool_redis::Config::from_url(format!("redis://{host}:{port}"));

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
    /// Returns the order in which this specific key was received (1 for first time).
    pub async fn try_key(&mut self, key: u32) -> Result<i64, CacheError> {
        cache_info!("(try_key) entry with key {}.", key);

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
