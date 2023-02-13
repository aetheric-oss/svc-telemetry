pub use mavlink::{MavFrame, MavHeader, MavlinkVersion, Message};
pub use mavlink::common::{MavMessage, ADSB_VEHICLE_DATA};
pub use adsb_deku::{Frame, DF};

pub trait Keys {
    /// Often the aircraft ID
    fn primary_key(&self) -> u32;

    /// The sequence number, timestamp, or checksum
    fn secondary_key(&self) -> u32;

    fn hashed_key(&self) -> u32 {
        let p = self.primary_key();

        // p*(large odd number) + s
        // better than bitwise XOR for avoiding collisions
        (p << 4) + p + self.secondary_key()
    }
}

impl Keys for MavHeader
{
    fn primary_key(&self) -> u32 {
        self.system_id as u32
    }

    fn secondary_key(&self) -> u32 {
        self.sequence as u32
    }
}

impl Keys for adsb_deku::Frame
{
    fn primary_key(&self) -> u32 {
        let bytes: [u8; 4] = match &self.df {
            adsb_deku::DF::ADSB(adsb) => {
                let mut bytes = [0; 4];
                bytes[1..4].copy_from_slice(&adsb.icao.0);
                bytes
            }
            // TODO this shouldn't be reached. handle
            _ => [0; 4]
        };

        u32::from_be_bytes(bytes)
    }

    fn secondary_key(&self) -> u32 {
        self.crc
    }
}
