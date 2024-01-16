//! gRPC
//! provides Redis implementations for caching layer

#[macro_use]
pub mod macros;
pub mod pool;

/// Wrapper struct for our Redis Pools
#[derive(Clone, Debug)]
pub struct RedisPools {
    /// Network Remote ID pool
    pub netrid: pool::RedisPool,
    /// Mavlink pool
    pub mavlink: pool::RedisPool,
    /// ADSB pool
    pub adsb: pool::RedisPool,
}
