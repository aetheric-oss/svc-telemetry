use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Adsb Packet wraps an adsb message and adds sender node UUID
#[allow(dead_code)]
#[derive(Debug, Clone, IntoParams, ToSchema)]
#[derive(Deserialize, Serialize)]
pub struct AdsbPacket {
    /// Asset ID
    pub sender_uuid: String,

    /// ads-b packet contents
    pub adsb: Vec<u8>,
}
