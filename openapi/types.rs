pub use adsb_deku::{Frame, DF};

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

impl Keys for Frame {
    fn primary_key(&self) -> u32 {
        let bytes: [u8; 4] = match &self.df {
            adsb_deku::DF::ADSB(adsb) => {
                let mut bytes = [0; 4];
                bytes[1..4].copy_from_slice(&adsb.icao.0);
                bytes
            }
            // TODO(R4): this shouldn't be reached. handle
            _ => [0; 4],
        };

        u32::from_be_bytes(bytes)
    }

    fn secondary_key(&self) -> u32 {
        self.crc
    }
}
