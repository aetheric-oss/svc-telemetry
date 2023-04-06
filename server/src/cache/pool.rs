use deadpool_redis::{redis, Pool, Runtime};
use snafu::prelude::Snafu;

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

        match config.redis.create_pool(Some(Runtime::Tokio1)) {
            Ok(pool) => Ok(RedisPool {
                pool,
                expiration_ms,
            }),
            Err(e) => {
                cache_error!("(RedisPool new) could not create pool: {}", e);
                Err(CacheError::CouldNotConfigure)
            }
        }
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

        let mut result = match redis::pipe()
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
            .await
        {
            Ok(redis::Value::Bulk(val)) => val,
            Ok(value) => {
                cache_error!("(try_key) Operation failed, unexpected redis response.");
                cache_debug!(
                    "(try_key) Operation failed, unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                cache_error!("(try_key) Operation failed, redis error.");
                cache_debug!("(try_key) Operation failed, redis error: {}", e);
                return Err(CacheError::OperationFailed);
            }
        };

        let new_value = match result.pop() {
            Some(redis::Value::Int(new_value)) => new_value,
            Some(value) => {
                cache_info!("(try_key) Operation failed, unexpected redis response.");
                cache_debug!(
                    "(try_key) Operation failed, unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            None => {
                cache_error!("(try_key) Operation failed, empty redis response array.");
                return Err(CacheError::OperationFailed);
            }
        };

        Ok(new_value)
    }
}
