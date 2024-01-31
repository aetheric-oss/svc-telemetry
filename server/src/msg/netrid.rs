//! Network Remote ID

use chrono::{DateTime, Utc};
use packed_struct::prelude::packed_bits::Bits;
use packed_struct::prelude::*;

///////////////////////////////////////////////
// Field Enumerations
///////////////////////////////////////////////
///////////////////////////////
///////////////////
//////////

const REMID_PROTOCOL_VERSION: u8 = 0x2;

/// Remote ID Message Types
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum MessageType {
    Basic = 0x0,
    Location = 0x1,
    Authentication = 0x2,
    SelfId = 0x3,
    System = 0x4,
    OperatorId = 0x5,
    MessagePack = 0xF,
}

/// Unmanned Aircraft Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum UaType {
    Undeclared = 0x0,
    Aeroplane = 0x1,
    Rotorcraft = 0x2,
    Gyroplane = 0x3,
    HybridLift = 0x4,
    Ornithopter = 0x5,
    Glider = 0x6,
    Kite = 0x7,
    FreeBalloon = 0x8,
    CaptiveBalloon = 0x9,
    Airship = 0xA,
    Unpowered = 0xB, // free fall or parachute
    Rocket = 0xC,
    Tethered = 0xD, // powered aircraft
    GroundObstacle = 0xE,
    Other = 0xF,
}

/// Identification Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum IdType {
    None = 0x0,
    SerialNumber = 0x1,
    CaaAssigned = 0x2,
    UtmAssigned = 0x3,
    SpecificSession = 0x4,
}

/// Operation Status
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum OperationalStatus {
    Undeclared = 0x0,
    Ground = 0x1,
    Airborne = 0x2,
    Emergency = 0x3,
    RemoteIdSystemFailure = 0x4,
    // 0x5 - 0xF are reserved
}

/// Horizontal Accuracy (in meters)
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum HorizontalAccuracyMeters {
    Gte18520 = 0x0,
    Lt18520 = 0x1,
    Lt7408 = 0x2,
    Lt3704 = 0x3,
    Lt1852 = 0x4,
    Lt926 = 0x5,
    Lt555_6 = 0x6,
    Lt185_2 = 0x7,
    Lt92_6 = 0x8,
    Lt30 = 0x9,
    Lt10 = 0xA,
    Lt3 = 0xB,
    Lt1 = 0xC,
    // 0xD - 0xF are reserved
}

/// Vertical Accuracy (in meters)
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum VerticalAccuracyMeters {
    Gte150Unknown = 0x0,
    Lt150 = 0x1,
    Lt45 = 0x2,
    Lt25 = 0x3,
    Lt10 = 0x4,
    Lt3 = 0x5,
    Lt1 = 0x6,
    // 0x7 - 0xF are reserved
}

/// Speed Accuracy (in meters per second)
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum SpeedAccuracyMetersPerSecond {
    Gte10Unknown = 0x0,
    Lt10 = 0x1,
    Lt3 = 0x2,
    Lt1 = 0x3,
    Lt0_3 = 0x4,
    // 0x5 - 0xF are reserved
}

/// Operator Location Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum OperatorLocationSource {
    Takeoff = 0x0,
    Dynamic = 0x1,
    Fixed = 0x2,
}

/// Unmanned System Certification Region
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum UaClassification {
    Undeclared = 0x0,
    EuropeanUnion = 0x1,
    // 0x2 - 0xF are reserved
}

/// European Union UA Category
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum EuropeanUnionCategory {
    Undefined = 0x0,
    Open = 0x1,
    Specific = 0x2,
    Certified = 0x3,
    // 0x4 - 0xF are reserved
}

/// European Union UA Class
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum EuropeanUnionClass {
    Undefined = 0x0,
    Class0 = 0x1,
    Class1 = 0x2,
    Class2 = 0x3,
    Class3 = 0x4,
    Class4 = 0x5,
    Class5 = 0x6,
    Class6 = 0x7,
    // 0x8 - 0xF are reserved
}

/// Authentication Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum UaAuthenticationType {
    None = 0x0,
    UasIdSignature = 0x1,
    OperatorIdSignature = 0x2,
    MessageSetSignature = 0x3,
    NetworkRemoteId = 0x4,
    SpecificAuthMethod = 0x5,
    // 0x6 - 0x9 are reserved
    // 0xA - 0xF are available for private use
}

#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum HeightType {
    AboveTakeoff = 0x0,
    AboveGroundLevel = 0x1,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum EastWestDirection {
    East = 0x0, // <180
    West = 0x1, // >=180
}

#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum SpeedMultiplier {
    X0_25 = 0x0,
    X0_75 = 0x1,
}

///////////////////////////////////////////////
// Packet Frame
// Header (1 Byte), Message (24 Bytes)
///////////////////////////////////////////////
///////////////////////////////
///////////////////
//////////

/// Remote ID Packet Frame Header
#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(endian = "msb", bit_numbering = "msb0", size_bytes = "1")]
pub struct Header {
    /// Message Type (Mandatory)
    #[packed_field(size_bits = "4", ty = "enum")]
    pub message_type: MessageType,

    /// Protocol Version (Mandatory)
    ///  See [`REMID_PROTOCOL_VERSION`] for default
    #[packed_field(size_bits = "4")]
    pub protocol_version: u8,
}

impl Default for Header {
    fn default() -> Self {
        Header {
            message_type: MessageType::Basic,
            protocol_version: REMID_PROTOCOL_VERSION,
        }
    }
}

/// Remote ID Packet Frame
#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(bit_numbering = "msb0", endian = "msb")]
pub struct Frame {
    /// The frame header
    #[packed_field(size_bytes = "1")]
    pub header: Header,

    /// The message body
    pub message: [u8; 24],
}

///////////////////////////////////////////////
// Messages
///////////////////////////////////////////////
///////////////////////////////
///////////////////
//////////

/// Remote ID Messages
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    Basic(BasicMessage),
    Location(LocationMessage),
    // Authentication(AuthenticationMessage),
    // SelfId(SelfIdMessage),
    // System(SystemMessage),
    // OperatorId(OperatorIdMessage),
    // MessagePack(MessagePackMessage),
}
/// Remote ID Basic Message
#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(bit_numbering = "msb0", endian = "msb", size_bytes = "24")]
pub struct BasicMessage {
    #[packed_field(size_bits = "4", ty = "enum")]
    pub id_type: IdType,

    /// Packet Version Number (Mandatory)
    #[packed_field(size_bits = "4", ty = "enum")]
    pub ua_type: UaType,

    /// Telemetry or Command (Mandatory)
    pub uas_id: [u8; 20],

    // Reserved Field
    pub reserved: [u8; 3],
}

/// Remote ID Location Message
#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(bit_numbering = "msb0", endian = "msb", size_bytes = "24")]
pub struct LocationMessage {
    #[packed_field(size_bits = "4", ty = "enum")]
    pub operational_status: OperationalStatus,

    #[packed_field(size_bits = "1")]
    pub reserved_0: Integer<u8, Bits<1>>,

    #[packed_field(size_bits = "1", ty = "enum")]
    pub height_type: HeightType,

    #[packed_field(size_bits = "1", ty = "enum")]
    pub ew_direction: EastWestDirection,

    #[packed_field(size_bits = "1", ty = "enum")]
    pub speed_multiplier: SpeedMultiplier,

    /// Track Direction measured clockwise from true North
    /// Add 180 to this value if EW Direction bit is set to 1 (facing west)
    /// (10 with EW Direction bit set to 0) == 10
    /// (10 with EW Direction bit set to 1) == 190
    #[packed_field(size_bytes = "1")]
    pub track_direction: u8,

    /// Encoded speed in meters per second
    #[packed_field(size_bytes = "1")]
    pub speed: u8,

    /// Encoded vertical rate in meters per second  
    /// + == up, - == down
    #[packed_field(size_bytes = "1")]
    pub vertical_speed: i8,

    /// Latitude
    #[packed_field(size_bytes = "4", endian = "lsb")]
    pub latitude: i32,

    /// Longitude
    #[packed_field(size_bytes = "4", endian = "lsb")]
    pub longitude: i32,

    /// Pressure altitude
    #[packed_field(size_bytes = "2", endian = "lsb")]
    pub pressure_altitude: u16,

    /// Geodetic altitude
    #[packed_field(size_bytes = "2", endian = "lsb")]
    pub geodetic_altitude: u16,

    /// Height above takeoff or ground (see Height Type bit)
    #[packed_field(size_bytes = "2", endian = "lsb")]
    pub height: u16,

    /// Vertical Accuracy
    #[packed_field(size_bits = "4", ty = "enum")]
    pub vertical_accuracy: VerticalAccuracyMeters,

    /// Horizontal Accuracy
    #[packed_field(size_bits = "4", ty = "enum")]
    pub horizontal_accuracy: HorizontalAccuracyMeters,

    /// Barometric Altitude
    #[packed_field(size_bits = "4", ty = "enum")]
    pub barometric_altitude_accuracy: VerticalAccuracyMeters,

    /// Speed Accuracy
    #[packed_field(size_bits = "4", ty = "enum")]
    pub speed_accuracy: SpeedAccuracyMetersPerSecond,

    /// Timestamp
    #[packed_field(size_bytes = "2", endian = "lsb")]
    pub timestamp: u16,

    /// Reserved Field
    #[packed_field(size_bits = "4")]
    pub reserved_1: Integer<u8, Bits<4>>,

    /// Timestamp Accuracy
    /// Values 0-15
    /// 0 = Unknown
    /// Multiply value by 0.1 seconds for accuracy
    /// (possible values then 0.1 -> 1.5)
    #[packed_field(size_bits = "4")]
    pub timestamp_accuracy: Integer<u8, Bits<4>>,

    #[packed_field(size_bytes = "1")]
    pub reserved_2: u8,
}

#[derive(PartialEq, Debug)]
pub enum LocationDecodeError {
    SpeedGte254_25,
    UnknownSpeed,
    UnknownAltitude,
}

impl LocationMessage {
    pub fn decode_direction(&self) -> u16 {
        match self.ew_direction {
            EastWestDirection::East => self.track_direction as u16,
            EastWestDirection::West => self.track_direction as u16 + 180,
        }
    }

    pub fn decode_altitude(&self) -> Result<f32, LocationDecodeError> {
        let altitude = (self.pressure_altitude as f32 * 0.5) - 1000.0;
        match altitude {
            -1000.0 => Err(LocationDecodeError::UnknownAltitude),
            x => Ok(x),
        }
    }

    pub fn decode_speed(&self) -> Result<f32, LocationDecodeError> {
        // Speed addition is added when the speed multiplier is 0.75
        //  0.75 is used when speed exceeds 63.75 m/s
        static HIGH_SPEED_ADDITION: f32 = 63.75; // (255.0 * 0.25);

        let speed = match self.speed_multiplier {
            SpeedMultiplier::X0_25 => self.speed as f32 * 0.25,
            SpeedMultiplier::X0_75 => (self.speed as f32 * 0.75) + HIGH_SPEED_ADDITION,
        };

        match speed {
            255.0 => Err(LocationDecodeError::UnknownSpeed),
            254.25 => Err(LocationDecodeError::SpeedGte254_25),
            x => Ok(x),
        }
    }

    pub fn decode_vertical_speed(&self) -> f32 {
        self.vertical_speed as f32 * 0.5
    }

    pub fn decode_latitude(&self) -> f64 {
        self.latitude as f64 * 1e-7
    }

    pub fn decode_longitude(&self) -> f64 {
        self.longitude as f64 * 1e-7
    }

    pub fn decode_timestamp(&self, receipt_timestamp: DateTime<Utc>) -> u16 {
        todo!("decode_timestamp")
    }

    // TODO(R5) encode implementations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_id_message() {
        let msg = BasicMessage {
            id_type: IdType::CaaAssigned,
            ua_type: UaType::Rotorcraft,
            uas_id: [0; 20],
            reserved: [0; 3],
        };

        let frame = Frame {
            header: Header {
                message_type: MessageType::Basic,
                ..Default::default()
            },
            message: msg.pack().unwrap(),
        };

        let bytes = frame.pack().unwrap();
        assert_eq!(bytes.len(), 25);
    }

    #[test]
    fn test_location_message() {
        let msg = LocationMessage {
            operational_status: OperationalStatus::Airborne,
            reserved_0: 0.into(),
            height_type: HeightType::AboveTakeoff,
            ew_direction: EastWestDirection::East,
            speed_multiplier: SpeedMultiplier::X0_25,
            track_direction: 10,
            speed: 0,
            vertical_speed: 0,
            latitude: 0,
            longitude: 0,
            pressure_altitude: 0,
            geodetic_altitude: 0,
            height: 0,
            vertical_accuracy: VerticalAccuracyMeters::Lt150,
            horizontal_accuracy: HorizontalAccuracyMeters::Lt1852,
            barometric_altitude_accuracy: VerticalAccuracyMeters::Lt150,
            speed_accuracy: SpeedAccuracyMetersPerSecond::Lt10,
            timestamp: 0,
            reserved_1: 0.into(),
            timestamp_accuracy: 0.into(),
            reserved_2: 0,
        };

        let frame = Frame {
            header: Header {
                message_type: MessageType::Location,
                ..Default::default()
            },
            message: msg.pack().unwrap(),
        };

        let bytes = frame.clone().pack().unwrap();
        assert_eq!(bytes.len(), 25);
    }

    #[test]
    fn test_location_decode() {
        let mut msg = LocationMessage {
            operational_status: OperationalStatus::Airborne,
            reserved_0: 0.into(),
            height_type: HeightType::AboveTakeoff,
            ew_direction: EastWestDirection::East,
            speed_multiplier: SpeedMultiplier::X0_25,
            track_direction: 10,
            speed: 30,
            vertical_speed: 0,
            latitude: -123456789,
            longitude: 123456789,
            pressure_altitude: 0,
            geodetic_altitude: 0,
            height: 0,
            vertical_accuracy: VerticalAccuracyMeters::Lt150,
            horizontal_accuracy: HorizontalAccuracyMeters::Lt1852,
            barometric_altitude_accuracy: VerticalAccuracyMeters::Lt150,
            speed_accuracy: SpeedAccuracyMetersPerSecond::Lt10,
            timestamp: 0,
            reserved_1: 0.into(),
            timestamp_accuracy: 0.into(),
            reserved_2: 0,
        };

        // Direction
        assert_eq!(msg.decode_direction(), 10);
        msg.ew_direction = EastWestDirection::West;
        assert_eq!(msg.decode_direction(), 190);

        // Altitude
        msg.pressure_altitude = 0;
        assert_eq!(
            msg.decode_altitude(),
            Err(LocationDecodeError::UnknownAltitude)
        );
        msg.pressure_altitude = 1000;
        assert_eq!(msg.decode_altitude(), Ok(-500.0));
    }
}
