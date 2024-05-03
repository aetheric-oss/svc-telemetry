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
    bytes
        .iter()
        .fold("".to_string(), |acc, byte| format!("{acc}{:02x}", byte))
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
