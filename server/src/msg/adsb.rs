//! Functions for parsing ADS-B packets

use adsb_deku::Sign;

/// Expected size of ADSB packets
pub const ADSB_SIZE_BYTES: usize = 14;

/// Possible errors decoding ADSB packets
#[derive(Debug, Copy, Clone)]
pub enum DecodeError {
    /// The latitudes of a packet pair are in different zones
    CrossedLatitudeZones,

    /// Invalid Aircraft Subtype (subtype is not 1 or 2)
    InvalidSubtype,
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
    let mut denominator = 1. - x;

    // acos is undefined for values outside of [-1, 1]
    if denominator < -1. {
        denominator = -1.;
    } else if denominator > 1. {
        denominator = 1.;
    }

    denominator = denominator.acos();

    //
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
    fn ut_number_of_longitude_zones() {
        assert_eq!(nl(0.), 59.);
        assert_eq!(nl(87.), 2.);
        assert_eq!(nl(-87.), 2.);
        // assert_eq!(nl(87.1), 1.); TODO(R4) incorrect around the poles
        // assert_eq!(nl(-87.1), 1.); TODO(R4) switch to lookup table
    }

    #[test]
    /// See 3.3 Latitude/Longitude calculation of https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf
    fn ut_decode_cpr() {
        //
        // Newest packet - even
        let lat_even = 0b10110101101001000;
        let lon_even = 0b01100100010101100;

        //
        // older packet - odd
        let lat_odd = 0b10010000110101110;
        let lon_odd = 0b01100010000010010;
        let (latitude, longitude) = decode_cpr(lat_even, lon_even, lat_odd, lon_odd).unwrap();

        println!("(ut_decode_cpr) lat: {}, lon: {}", latitude, longitude);
        assert!((latitude - 52.25720214843750).abs() < 0.0000001);
        assert!((longitude - 3.91937).abs() < 0.0001);
    }

    #[test]
    fn ut_decode_altitude() {
        let alt = 0b110000111000;
        let expected_ft: f32 = 38000.;
        let expected_meters = expected_ft * 0.3048;
        let altitude = decode_altitude(alt);

        assert!((altitude - expected_meters).abs() < 0.001);
    }

    #[test]
    fn ut_decode_vertical_speed() {
        let speed = decode_vertical_speed(Sign::Negative, 14).unwrap();
        let expected_speed = -832.0 * 0.3048; // ftps -> m/s

        println!("(ut_decode_vertical_speed) speed: {speed}",);
        assert!((speed - expected_speed).abs() < 0.01);

        let speed = decode_vertical_speed(Sign::Negative, 37).unwrap();
        let expected_speed = -2304.0 * 0.3048; // ftps -> m/s

        println!("(ut_decode_vertical_speed) speed: {speed}",);
        assert!((speed - expected_speed).abs() < 0.01);
    }

    #[test]
    fn ut_decode_speed_direction() {
        let (speed, direction) =
            decode_speed_direction(1, Sign::Negative, 9, Sign::Negative, 160).unwrap();

        let expected_speed = 159.20 * 0.514444; // knots -> m/s
        let expected_angle = 182.88;

        println!(
            "(ut_decode_speed_direction) speed: {}, direction: {}",
            speed, direction
        );
        assert!((speed - expected_speed).abs() < 0.01);
        assert!((direction - expected_angle).abs() < 0.01);
    }
}
