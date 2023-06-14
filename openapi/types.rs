pub use adsb_deku::{Frame, DF};
pub use mavlink::common::{MavMessage, ADSB_VEHICLE_DATA};
pub use mavlink::{MavFrame, MavHeader, MavlinkVersion, Message};

/// A trait for getting a hashed key from a bit-packed frame
pub trait Keys {
    /// Often the aircraft ID
    fn primary_key(&self) -> u32;

    /// The sequence number, timestamp, or checksum
    fn secondary_key(&self) -> u32;

    /// A key combining the primary and secondary keys
    fn hashed_key(&self) -> u32 {
        let p = self.primary_key();

        // p*(large odd number) + s
        // better than bitwise XOR for avoiding collisions
        (p << 4) + p + self.secondary_key()
    }
}

impl Keys for MavHeader {
    fn primary_key(&self) -> u32 {
        self.system_id as u32
    }

    fn secondary_key(&self) -> u32 {
        self.sequence as u32
    }
}

impl Keys for Frame {
    fn primary_key(&self) -> u32 {
        let bytes: [u8; 4] = match &self.df {
            adsb_deku::DF::ADSB(adsb) => {
                let mut bytes = [0; 4];
                bytes[1..4].copy_from_slice(&adsb.icao.0);
                bytes
            }
            // TODO this shouldn't be reached. handle
            _ => [0; 4],
        };

        u32::from_be_bytes(bytes)
    }

    fn secondary_key(&self) -> u32 {
        self.crc
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashed_key_mavheader() {
        let mav_header = MavHeader {
            system_id: 42,
            component_id: 10,
            sequence: 20,
        };

        let hashed_key = mav_header.hashed_key();

        // Perform assertions on the hashed key
        // Replace the expected_hashed_key value with the actual expected result
        let expected_hashed_key = (42 << 4) + 42 + 20;
        assert_eq!(hashed_key, expected_hashed_key);
    }
}
