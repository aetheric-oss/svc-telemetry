/// Functions for parsing ADS-B packets
use adsb_deku::Sign;
use std::fmt::{self, Display, Formatter};

/// Expected size of ADSB packets
pub const ADSB_SIZE_BYTES: usize = 14;

/// Possible errors decoding ADSB packets
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DecodeError {
    /// The latitudes of a packet pair are in different zones
    CrossedLatitudeZones,

    /// Unsupported Subtype
    UnsupportedSubtype,

    /// Invalid Aircraft Subtype (subtype is not 1, 2, 3, 4)
    InvalidSubtype,
}

/// Possible errors encoding ADSB packets
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EncodeError {
    /// Invalid CPR flag
    InvalidFlag,
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::CrossedLatitudeZones => write!(f, "Crossed latitude zones"),
            DecodeError::UnsupportedSubtype => write!(f, "Unsupported subtype"),
            DecodeError::InvalidSubtype => write!(f, "Invalid subtype"),
        }
    }
}

impl Display for EncodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::InvalidFlag => write!(f, "Invalid CPR flag"),
        }
    }
}

/// Convert the ICAO field to a u32
pub fn get_adsb_icao_address(icao: &[u8; 3]) -> u32 {
    let mut bytes = [0; 4];
    bytes[1..4].copy_from_slice(icao);
    u32::from_be_bytes(bytes)
}

/// Parses the ADS-B packet for the message type filed
/// Bits 32-37 (0-index)
pub fn get_adsb_message_type(bytes: &[u8; ADSB_SIZE_BYTES]) -> i64 {
    // First 5 bits of the fifth byte
    ((bytes[4] >> 3) & 0x1F) as i64
}

/// Converts an encoded ADS-B altitude to altitude in meters
pub fn decode_altitude(altitude: u16) -> f32 {
    // Bit 48 indicates if the altitude is encoded in multiples of
    //  25 or 100 feet
    let altitude: u32 = altitude as u32;
    let coef_ft: u32 = if (0x010 & altitude) > 0 { 25 } else { 100 };

    // Ignore bit 48 (bit 8 of the altitude field)
    let n: u32 = ((0xFE0 & altitude) >> 1) | (0xF & altitude);
    let alt_ft = n * coef_ft - 1000;
    0.3048 * alt_ft as f32
}

/// Encode the altitude in the ADS-B packet
pub fn encode_altitude(altitude_m: f32) -> u16 {
    let altitude: u16 = (altitude_m / 0.3048) as u16;
    let altitude = (altitude + 1000) / 25;
    let qbit = 1; // increments of 25 feet

    // shift top 8 bits over by 1, add in the Q-bit, then OR with the bottom 4
    ((altitude & 0xFFF0) << 1) | (qbit << 4) | (altitude & 0xF)
}

///
/// Returns the remainder after dividing x by y
fn modulus(x: f64, y: f64) -> f64 {
    x - y * ((x / y).floor())
}

///
/// Finds the number of longitude zones, given a latitude angle
///
/// Assuming number of zones (NZ) is 15 for Mode-S CPR encoding.
fn nl(lat: f64) -> f64 {
    use std::f64::consts::PI;
    const NZ: f64 = 30.; // NZ * 2

    //
    // Numerator
    let numerator: f64 = 2. * PI;

    //
    // Denominator
    let a = 1. - (PI / NZ).cos();
    let b = (1. + (2. * (PI * lat / 180.)).cos()) / 2.;
    let x = a / b;
    let denominator = (1. - x)
        // acos is undefined for values outside of [-1, 1]
        .clamp(-1., 1.)
        .acos();

    // Result
    let result = numerator / denominator;
    // println!("(nl) result: {} (num: {}, denom: {})", result, numerator, denominator);
    result.floor()
}

/// Decodes the CPR format
/// <https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf>
pub fn decode_cpr(
    lat_cpr_even: u32,
    lon_cpr_even: u32,
    lat_cpr_odd: u32,
    lon_cpr_odd: u32,
) -> Result<(f64, f64), DecodeError> {
    let lat_cpr_even: f64 = lat_cpr_even as f64 / 131072.;
    let lon_cpr_even: f64 = lon_cpr_even as f64 / 131072.;
    let lat_cpr_odd: f64 = lat_cpr_odd as f64 / 131072.;
    let lon_cpr_odd: f64 = lon_cpr_odd as f64 / 131072.;
    let lat_index: f64 = (59. * lat_cpr_even - 60. * lat_cpr_odd + 0.5).floor();
    let dlat_even = 6.0; // 360. / 60.;
    let dlat_odd = 6.101694915254237; // 360. / 59.

    //
    // Compute Latitude
    let mut lat_even: f64 = dlat_even * (lat_cpr_even + modulus(lat_index, 60.));
    let mut lat_odd: f64 = dlat_odd * (lat_cpr_odd + modulus(lat_index, 59.));

    if lat_even >= 270. {
        lat_even -= 360.;
    }

    if lat_odd >= 270. {
        lat_odd -= 360.;
    }

    let latitude: f64 = lat_even; // We trigger on receiving the odd packet
    let nl_le: f64 = nl(lat_even);
    let nl_lo: f64 = nl(lat_odd);

    if nl_le != nl_lo {
        return Err(DecodeError::CrossedLatitudeZones);
    }

    //
    // Compute Longitude
    let ni = if nl_le < 1. { 1. } else { nl_le };

    let dlon: f64 = 360. / ni;
    let m: f64 = (lon_cpr_even * (nl_le - 1.) - lon_cpr_odd * nl_le + 0.5).floor();
    let mut longitude: f64 = dlon * (modulus(m, ni) + lon_cpr_even);

    if longitude >= 180. {
        longitude -= 360.;
    }

    Ok((latitude, longitude))
}

/// Encodes latitude and longitude in CPR format
/// <https://mode-s.org/decode/content/ads-b/3-airborne-position.html#cpr-zones>
pub fn encode_cpr(cpr_flag: u8, longitude: f64, latitude: f64) -> Result<(u32, u32), EncodeError> {
    static SCALAR: f64 = 2u32.pow(17) as f64;
    let i = match cpr_flag {
        0 => 0., // even
        1 => 1., // odd
        _ => return Err(EncodeError::InvalidFlag),
    };

    let dlat = 360. / (60. - i);
    let yz = (SCALAR * modulus(latitude, dlat) / dlat + 0.5).floor();
    let cpr_latitude = modulus(yz, SCALAR);
    let rlat = dlat * ((latitude / dlat).floor() + cpr_latitude / SCALAR);
    let dlon = 360. / 1.0_f64.max(nl(rlat) - i);
    let xz = (SCALAR * modulus(longitude, dlon) / dlon + 0.5).floor();
    let cpr_longitude = modulus(xz, SCALAR);

    Ok((cpr_longitude as u32, cpr_latitude as u32))
}

/// Decodes the speed and direction of an aircraft
/// <https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf>
pub fn decode_speed_direction(
    st: u8,
    ew_sign: Sign,
    ew_vel: u16,
    ns_sign: Sign,
    ns_vel: u16,
) -> Result<(f32, f32), DecodeError> {
    use std::f32::consts::PI;
    static DIRECTION_COEFFICIENT: f32 = 360. / (2. * PI);

    // Sign: 0 = positive, 1 = negative
    let ew_vel: i32 = ew_vel as i32;
    let ns_vel: i32 = ns_vel as i32;

    let (vx, vy) = match st {
        1 => {
            let vx = match ew_sign {
                Sign::Positive => ew_vel - 1,
                Sign::Negative => -(ew_vel - 1),
            };

            let vy = match ns_sign {
                Sign::Positive => ns_vel - 1,
                Sign::Negative => -(ns_vel - 1),
            };

            (vx, vy)
        }
        2 => {
            let vx = match ew_sign {
                Sign::Positive => 4 * (ew_vel - 1),
                Sign::Negative => -4 * (ew_vel - 1),
            };

            let vy = match ns_sign {
                Sign::Positive => 4 * (ns_vel - 1),
                Sign::Negative => -4 * (ns_vel - 1),
            };

            (vx, vy)
        }
        3 | 4 => return Err(DecodeError::UnsupportedSubtype),
        _ => return Err(DecodeError::InvalidSubtype),
    };

    let speed_knots = ((vx.pow(2) + vy.pow(2)) as f32).sqrt();
    let speed_mps = speed_knots * 0.514444;
    let mut direction = (vx as f32).atan2(vy as f32) * DIRECTION_COEFFICIENT;

    if direction < 0. {
        direction += 360.;
    }

    Ok((speed_mps, direction))
}

/// Decodes the vertical speed of an aircraft
/// <https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf>
pub fn decode_vertical_speed(vrate_sign: Sign, vrate_value: u16) -> Result<f32, DecodeError> {
    // Sign: positive = 0, negative = 1
    // 0 = climb, 1 = descend
    let vrate_value = vrate_value as i32;
    let speed_ftps = match vrate_sign {
        Sign::Positive => 64 * (vrate_value - 1),
        Sign::Negative => -64 * (vrate_value - 1),
    };

    let speed_mps = speed_ftps as f32 * 0.3048;
    Ok(speed_mps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// See 3.2.4 NL(lat) of https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf
    fn test_number_of_longitude_zones() {
        assert_eq!(nl(0.), 59.);
        assert_eq!(nl(87.), 2.);
        assert_eq!(nl(-87.), 2.);
        // assert_eq!(nl(87.1), 1.); TODO(R5) incorrect around the poles
        // assert_eq!(nl(-87.1), 1.); TODO(R5) switch to lookup table
    }

    #[test]
    /// See 3.3 Latitude/Longitude calculation of https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf
    fn test_decode_cpr() {
        //
        // Newest packet - even
        let lat_even = 0b10110101101001000;
        let lon_even = 0b01100100010101100;

        //
        // older packet - odd
        let lat_odd = 0b10010000110101110;
        let lon_odd = 0b01100010000010010;
        let (latitude, longitude) = decode_cpr(lat_even, lon_even, lat_odd, lon_odd).unwrap();

        println!("(test_decode_cpr) lat: {}, lon: {}", latitude, longitude);
        assert!((latitude - 52.25720214843750).abs() < 0.0000001);
        assert!((longitude - 3.91937).abs() < 0.0001);
    }

    #[test]
    fn test_decode_altitude() {
        let alt = 0b110000111000;
        let expected_ft: f32 = 38000.;
        let expected_meters = expected_ft * 0.3048;
        let altitude = decode_altitude(alt);

        assert!((altitude - expected_meters).abs() < 0.001);
    }

    #[test]
    fn test_decode_vertical_speed() {
        let speed = decode_vertical_speed(Sign::Negative, 14).unwrap();
        let expected_speed = -832.0 * 0.3048; // ftps -> m/s
        assert!((speed - expected_speed).abs() < 0.01);

        let speed = decode_vertical_speed(Sign::Negative, 37).unwrap();
        let expected_speed = -2304.0 * 0.3048; // ftps -> m/s
        assert!((speed - expected_speed).abs() < 0.01);

        let speed = decode_vertical_speed(Sign::Positive, 37).unwrap();
        let expected_speed = expected_speed * -1.;
        assert!((speed - expected_speed).abs() < 0.01);
    }

    #[test]
    fn test_decode_speed_direction() {
        // subtype 1 (subsonic)
        let (speed, direction) =
            decode_speed_direction(1, Sign::Negative, 9, Sign::Negative, 160).unwrap();

        let expected_speed = 159.20 * 0.514444; // knots -> m/s
        let mut expected_angle = 182.88;
        assert!((speed - expected_speed).abs() < 0.01);
        assert!((direction - expected_angle).abs() < 0.01);

        // changing north to south and east to west shouldn't change the speed
        // angle should be opposite of the original
        let (speed, direction) =
            decode_speed_direction(1, Sign::Positive, 9, Sign::Positive, 160).unwrap();
        expected_angle -= 180.;
        assert!((speed - expected_speed).abs() < 0.01);
        assert!((direction - expected_angle).abs() < 0.01);

        // subtype 2 (supersonic)
        // change north to south
        // flip angle over the one axis
        let (supersonic_speed, direction) =
            decode_speed_direction(2, Sign::Positive, 9, Sign::Negative, 160).unwrap();
        assert!(supersonic_speed - (speed * 4.0) < 0.01);
        expected_angle = 180. - expected_angle; // flips angle North to South
        assert!((direction - expected_angle).abs() < 0.01);

        let (supersonic_speed, direction) =
            decode_speed_direction(2, Sign::Negative, 9, Sign::Positive, 160).unwrap();
        expected_angle += 180.;
        assert!(supersonic_speed - (speed * 4.0) < 0.01);
        assert!((direction - expected_angle).abs() < 0.01);

        // unsupported subtype
        // airspeed only (no groundspeed)
        // subsonic
        let error = decode_speed_direction(3, Sign::Negative, 9, Sign::Negative, 160).unwrap_err();
        assert_eq!(error, DecodeError::UnsupportedSubtype);
        // supersonic
        let error = decode_speed_direction(4, Sign::Negative, 9, Sign::Negative, 160).unwrap_err();
        assert_eq!(error, DecodeError::UnsupportedSubtype);

        // Invalid subtype
        let error = decode_speed_direction(5, Sign::Negative, 9, Sign::Negative, 160).unwrap_err();
        assert_eq!(error, DecodeError::InvalidSubtype);
    }

    #[test]
    fn test_get_adsb_icao_address() {
        let icao = [0x01, 0x02, 0x03];
        let icao_address = get_adsb_icao_address(&icao);
        assert_eq!(icao_address, 0x00010203);
    }

    #[test]
    fn test_get_adsb_message_type() {
        let bytes = [0; ADSB_SIZE_BYTES];
        let message_type = get_adsb_message_type(&bytes);
        assert_eq!(message_type, 0);

        let mut bytes = [0; ADSB_SIZE_BYTES];
        let expected_message_type = 0b10101;
        bytes[4] = expected_message_type << 3;

        let message_type = get_adsb_message_type(&bytes);
        assert_eq!(message_type, expected_message_type as i64);
    }

    #[test]
    fn test_decode_error_display() {
        assert_eq!(
            DecodeError::CrossedLatitudeZones.to_string(),
            "Crossed latitude zones"
        );
        assert_eq!(DecodeError::InvalidSubtype.to_string(), "Invalid subtype");
    }

    #[test]
    fn test_encode_altitude() {
        let altitude_ft: f32 = 38_000.0;
        let altitude_m: f32 = altitude_ft * 0.3048;
        let expected_encoded = 0b110000111000;
        let encoded = encode_altitude(altitude_m);
        assert_eq!(encoded, expected_encoded);
    }

    #[test]
    fn test_encode_cpr() {
        // Use example from 1090MHz
        // <https://mode-s.org/decode/content/ads-b/3-airborne-position.html#decoding-example>
        let cpr_flag = 0;
        let longitude = 3.91937255859375;
        let latitude = 52.2572021484375;

        let scalar: f64 = 2u32.pow(17) as f64;
        let i = match cpr_flag {
            0 => 0., // even
            1 => 1., // odd
            _ => panic!(),
        };

        let dlat = 360. / (60. - i);
        let yz = (scalar * modulus(latitude, dlat) / dlat + 0.5).floor();
        let cpr_latitude = modulus(yz, scalar);
        let rlat = dlat * ((latitude / dlat).floor() + cpr_latitude / scalar);
        let dlon = 360. / 1.0_f64.max(nl(rlat) - i);

        let (cpr_longitude, cpr_latitude) = encode_cpr(cpr_flag, longitude, latitude).unwrap();
        let expected_longitude_cpr = 0b01100100010101100;
        let expected_latitude_cpr = 0b10110101101001000;
        let tolerance_latitude = dlat / (2_i32.pow(18) as f64);
        let tolerance_longitude = dlon / (2_i32.pow(18) as f64);

        println!(
            "(test_encode_cpr) lat: {}, lon: {}",
            cpr_latitude, cpr_longitude
        );
        println!(
            "(test_encode_cpr) expected lat: {}, expected lon: {}",
            expected_latitude_cpr, expected_longitude_cpr
        );
        println!(
            "(test_encode_cpr) tolerance lat: {}, tolerance lon: {}",
            tolerance_latitude, tolerance_longitude
        );

        assert!((expected_latitude_cpr as f64 - cpr_latitude as f64).abs() < tolerance_latitude);
        assert!((expected_longitude_cpr as f64 - cpr_longitude as f64).abs() < tolerance_longitude);
    }
}
