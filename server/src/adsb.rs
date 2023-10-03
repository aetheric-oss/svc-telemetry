//! Functions for parsing ADS-B packets

use std::f64::consts::PI;

/// Expected size of ADSB packets
pub const ADSB_SIZE_BYTES: usize = 14;

/// Possible errors decoding ADSB packets
#[derive(Debug, Copy, Clone)]
pub enum DecodeError {
    /// The latitudes of a packet pair are in different zones
    CrossedLatitudeZones,
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
}
