use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
pub use mavlink::{MavFrame, MavHeader, MavlinkVersion, Message};
pub use mavlink::common::{MavMessage, ADSB_VEHICLE_DATA};

/// Contains the bytes of a Mavlink protocol message
#[derive(Serialize, Deserialize, ToSchema)]
pub struct MavlinkMessage {
    pub bytes: Vec<u8>
}

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
