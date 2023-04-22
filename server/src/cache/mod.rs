#[macro_use]
pub mod macros;
pub mod pool;

#[derive(Clone)]
pub struct RedisPools {
    pub mavlink: pool::RedisPool,
    pub adsb: pool::RedisPool,
}
