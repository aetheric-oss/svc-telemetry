//! gRPC
//! provides Redis implementations for caching layer

#[macro_use]
pub mod macros;
pub mod pool;

/// Wrapper struct for our Redis Pools
#[derive(Clone, Debug)]
pub struct TelemetryPools {
    /// Network Remote ID pool
    pub netrid: pool::TelemetryPool,
    /// ADSB pool
    pub adsb: pool::TelemetryPool,
}

/// Convert bytes to a key
pub fn bytes_to_key(bytes: &[u8]) -> String {
    let mut key = String::new();

    for byte in bytes {
        key.push_str(&format!("{:02x}", byte));
    }

    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_key() {
        let frame = vec![0x01, 0x02, 0x03, 0x04];
        let key = bytes_to_key(&frame);
        assert_eq!(key, "01020304");
    }
}
