/// Network Remote ID
use chrono::{DateTime, Utc};
use packed_struct::prelude::packed_bits::Bits;
use packed_struct::prelude::*;

///////////////////////////////////////////////
// Field Enumerations
///////////////////////////////////////////////
///////////////////////////////
///////////////////
//////////

/// Remote ID Protocol Version
pub const REMOTE_ID_PROTOCOL_VERSION: u8 = 0x2;

/// Remote ID Message Types
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum MessageType {
    /// Basic Remote ID Message
    Basic = 0x0,

    /// Location Remote ID Message
    Location = 0x1,

    /// Authentication Remote ID Message
    Authentication = 0x2,

    /// Self ID Remote ID Message
    SelfId = 0x3,

    /// System Remote ID Message
    System = 0x4,

    /// Operator ID Remote ID Message
    OperatorId = 0x5,

    /// Message Pack Remote ID Message
    MessagePack = 0xF,
}

/// Unmanned Aircraft Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum UaType {
    /// Unspecified
    Undeclared = 0x0,

    /// Aeroplane
    Aeroplane = 0x1,

    /// Rotorcraft
    Rotorcraft = 0x2,

    /// Gyroplane
    Gyroplane = 0x3,

    /// Hybrid Lift
    HybridLift = 0x4,

    /// Ornithopter
    Ornithopter = 0x5,

    /// Glider
    Glider = 0x6,

    /// Kite
    Kite = 0x7,

    /// Free Balloon
    FreeBalloon = 0x8,

    /// Captive Balloon
    CaptiveBalloon = 0x9,

    /// Airship
    Airship = 0xA,

    /// Unpowered (free fall or parachute)
    Unpowered = 0xB,

    /// Rocket
    Rocket = 0xC,

    /// Tethered (powered aircraft)
    Tethered = 0xD,

    /// Ground Obstacle (windmill, skyscraper, etc.)
    GroundObstacle = 0xE,

    /// Other
    Other = 0xF,
}

/// Identification Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum IdType {
    /// Unspecified
    None = 0x0,

    /// Serial Number
    SerialNumber = 0x1,

    /// Civil Aviation Authority Assigned
    CaaAssigned = 0x2,

    /// UTM Assigned
    UtmAssigned = 0x3,

    /// Specific Session
    SpecificSession = 0x4,
}

/// Operation Status
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum OperationalStatus {
    /// Unspecified
    Undeclared = 0x0,

    /// Ground
    Ground = 0x1,

    /// Airborne
    Airborne = 0x2,

    /// Emergency
    Emergency = 0x3,

    /// System Failure
    SystemFailure = 0x4,
    // 0x5 - 0xF are reserved
}

/// Horizontal Accuracy (in meters)
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum HorizontalAccuracyMeters {
    /// Greater than or equal to 18520 meters
    Gte18520 = 0x0,

    /// Less than 18520 meters
    Lt18520 = 0x1,

    /// Less than 7408 meters
    Lt7408 = 0x2,

    /// Less than 3704 meters
    Lt3704 = 0x3,

    /// Less than 1852 meters
    Lt1852 = 0x4,

    /// Less than 926 meters
    Lt926 = 0x5,

    /// Less than 555.6 meters
    Lt555_6 = 0x6,

    /// Less than 185.2 meters
    Lt185_2 = 0x7,

    /// Less than 92.6 meters
    Lt92_6 = 0x8,

    /// Less than 30 meters
    Lt30 = 0x9,

    /// Less than 10 meters
    Lt10 = 0xA,

    /// Less than 3 meters
    Lt3 = 0xB,

    /// Less than 1 meter
    Lt1 = 0xC,
    // 0xD - 0xF are reserved
}

/// Vertical Accuracy (in meters)
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum VerticalAccuracyMeters {
    /// Unknown, or greater than or equal to 150 meters
    Gte150Unknown = 0x0,

    /// Less than 150 meters
    Lt150 = 0x1,

    /// Less than 45 meters
    Lt45 = 0x2,

    /// Less than 25 meters
    Lt25 = 0x3,

    /// Less than 10 meters
    Lt10 = 0x4,

    /// Less than 3 meters
    Lt3 = 0x5,

    /// Less than 1 meter
    Lt1 = 0x6,
    // 0x7 - 0xF are reserved
}

/// Speed Accuracy (in meters per second)
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum SpeedAccuracyMetersPerSecond {
    /// Unknown, or greater than or equal to 10 meters per second
    Gte10Unknown = 0x0,

    /// Less than 10 meters per second
    Lt10 = 0x1,

    /// Less than 3 meters per second
    Lt3 = 0x2,

    /// Less than 1 meter per second
    Lt1 = 0x3,

    /// Less than 0.3 meters per second
    Lt0_3 = 0x4,
    // 0x5 - 0xF are reserved
}

/// Operator Location Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum OperatorLocationSource {
    /// Takeoff Location
    Takeoff = 0x0,

    /// Mobile operator location
    Dynamic = 0x1,

    /// Fixed operator location
    Fixed = 0x2,
}

/// Unmanned System Certification Region
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum UaClassification {
    /// Unspecified
    Undeclared = 0x0,

    /// EU (European Union) classification
    EuropeanUnion = 0x1,
    // 0x2 - 0xF are reserved
}

/// European Union UA Category
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum EuropeanUnionCategory {
    /// Unspecified
    Undefined = 0x0,

    /// Open Category
    Open = 0x1,

    /// Specific Category
    Specific = 0x2,

    /// Certified Category
    Certified = 0x3,
    // 0x4 - 0xF are reserved
}

/// European Union UA Class
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum EuropeanUnionClass {
    /// Unspecified
    Undefined = 0x0,

    //
    // CATEGORY A1
    // Not over assemblies of people
    /// Class 0 (< 250g MTOM)
    C0 = 0x1,

    /// Class 1 (< 900g MTOM)
    C1 = 0x2,

    //
    // CATEGORY A2
    // May fly close to people
    /// Class 2 (< 4kg MTOM)
    C2 = 0x3,

    //
    // CATEGORY A3
    // Fly far from people
    // < 25kg MTOM
    /// Class 3
    C3 = 0x4,

    /// Class 4
    C4 = 0x5,

    /// Class 5
    C5 = 0x6,

    /// Class 6
    C6 = 0x7,
    // 0x8 - 0xF are reserved
}

/// Authentication Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum UaAuthenticationType {
    /// Unspecified
    None = 0x0,

    /// UAS ID Signature
    UasIdSignature = 0x1,

    /// Operator ID Signature
    OperatorIdSignature = 0x2,

    /// Message Set Signature
    MessageSetSignature = 0x3,

    /// Network Remote ID
    NetworkRemoteId = 0x4,

    /// Specific Authentication Method
    SpecificAuthMethod = 0x5,
    // 0x6 - 0x9 are reserved
    // 0xA - 0xF are available for private use
}

/// Height Type
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum HeightType {
    /// Height Above Takeoff
    AboveTakeoff = 0x0,

    /// Height Above Ground Level
    AboveGroundLevel = 0x1,
}

/// East/West Direction
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum EastWestDirection {
    /// East (<180)
    East = 0x0,

    /// West (>=180)
    West = 0x1,
}

/// Speed Multiplier
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum SpeedMultiplier {
    /// Speed should be multiplied by 0.25 when decoded
    X0_25 = 0x0,

    /// Speed should be multiplied by 0.75 when decoded
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
    ///  See [`REMOTE_ID_PROTOCOL_VERSION`] for default
    #[packed_field(size_bits = "4")]
    pub protocol_version: u8,
}

impl Default for Header {
    fn default() -> Self {
        Header {
            message_type: MessageType::Basic,
            protocol_version: REMOTE_ID_PROTOCOL_VERSION,
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
    /// Remote ID Basic Message
    Basic(BasicMessage),

    /// Remote ID Location Message
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
    /// Identification Type (Mandatory)
    #[packed_field(size_bits = "4", ty = "enum")]
    pub id_type: IdType,

    /// Packet Version Number (Mandatory)
    #[packed_field(size_bits = "4", ty = "enum")]
    pub ua_type: UaType,

    /// Telemetry or Command (Mandatory)
    pub uas_id: [u8; 20],

    /// Reserved Field
    pub reserved: [u8; 3],
}

impl Default for BasicMessage {
    fn default() -> Self {
        BasicMessage {
            id_type: IdType::None,
            ua_type: UaType::Undeclared,
            uas_id: [0; 20],
            reserved: [0; 3],
        }
    }
}

/// Remote ID Location Message
#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(bit_numbering = "msb0", endian = "msb", size_bytes = "24")]
pub struct LocationMessage {
    /// Operational Status
    #[packed_field(size_bits = "4", ty = "enum")]
    pub operational_status: OperationalStatus,

    /// Reserved Field
    #[packed_field(size_bits = "1")]
    pub reserved_0: Integer<u8, Bits<1>>,

    /// Height Type
    #[packed_field(size_bits = "1", ty = "enum")]
    pub height_type: HeightType,

    /// East/West Direction
    #[packed_field(size_bits = "1", ty = "enum")]
    pub ew_direction: EastWestDirection,

    /// Speed Multiplier
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

    /// Reserved Field
    #[packed_field(size_bytes = "1")]
    pub reserved_2: u8,
}

/// Errors decoding a location message
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum LocationDecodeError {
    /// Speed is greater than or equal to 254.25 m/s
    SpeedGte254_25,

    /// Unknown speed
    UnknownSpeed,

    /// Unknown altitude
    UnknownAltitude,

    /// Unknown timestamp
    UnknownTimestamp,
}

impl LocationMessage {
    /// Decode the direction
    pub fn decode_direction(&self) -> u16 {
        match self.ew_direction {
            EastWestDirection::East => self.track_direction as u16,
            EastWestDirection::West => self.track_direction as u16 + 180,
        }
    }

    /// Decode the altitude
    pub fn decode_altitude(&self) -> Result<f32, LocationDecodeError> {
        let altitude = (self.pressure_altitude as f32 * 0.5) - 1000.0;

        if altitude == -1000.0 {
            return Err(LocationDecodeError::UnknownAltitude);
        }

        Ok(altitude)
    }

    /// Decode the speed in meters per second
    pub fn decode_speed(&self) -> Result<f32, LocationDecodeError> {
        // Speed addition is added when the speed multiplier is 0.75
        //  0.75 is used when speed exceeds 63.75 m/s
        static HIGH_SPEED_ADDITION: f32 = 63.75; // (255.0 * 0.25);

        let speed = match self.speed_multiplier {
            SpeedMultiplier::X0_25 => self.speed as f32 * 0.25,
            SpeedMultiplier::X0_75 => (self.speed as f32 * 0.75) + HIGH_SPEED_ADDITION,
        };

        if speed == 255.0 {
            Err(LocationDecodeError::UnknownSpeed)
        } else if speed == 254.25 {
            Err(LocationDecodeError::SpeedGte254_25)
        } else {
            Ok(speed)
        }
    }

    /// Decode the vertical speed in meters per second
    pub fn decode_vertical_speed(&self) -> Result<f32, LocationDecodeError> {
        let mut speed = (self.vertical_speed as f32) * 0.5;

        if speed == 63.0 {
            return Err(LocationDecodeError::UnknownSpeed);
        }

        if speed >= 62.0 {
            speed = 62.0;
        } else if speed <= -62.0 {
            speed = -62.0;
        }

        Ok(speed)
    }

    /// Decode the latitude
    pub fn decode_latitude(&self) -> f64 {
        self.latitude as f64 * 1e-7
    }

    /// Decode the longitude
    pub fn decode_longitude(&self) -> f64 {
        self.longitude as f64 * 1e-7
    }

    /// Decode the timestamp
    pub fn decode_timestamp(&self) -> Result<DateTime<Utc>, LocationDecodeError> {
        // TODO(R5): Aircraft timestamp decode
        Err(LocationDecodeError::UnknownTimestamp)
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
