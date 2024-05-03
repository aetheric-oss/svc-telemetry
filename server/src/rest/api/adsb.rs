//! Endpoints for updating aircraft positions

use crate::cache::pool::{GisPool, TelemetryPool};
use crate::cache::TelemetryPools;
use crate::grpc::client::GrpcClients;
use crate::msg::adsb::{
    decode_altitude, decode_cpr, decode_speed_direction, decode_vertical_speed,
    get_adsb_icao_address, get_adsb_message_type, ADSB_SIZE_BYTES,
};
use adsb_deku::adsb::ME::AirbornePositionBaroAltitude as AirbornePosition;
use adsb_deku::adsb::ME::AirborneVelocity as Velocity;
use adsb_deku::adsb::ME::AircraftIdentification as Identification;
use adsb_deku::adsb::{AirborneVelocitySubType, GroundSpeedDecoding, TypeCoding};
use adsb_deku::deku::DekuContainerRead;
use adsb_deku::{CPRFormat, Sign};
use svc_gis_client_grpc::prelude::types::*;
use svc_storage_client_grpc::prelude::*;
use svc_storage_client_grpc::resources::adsb;

use axum::{body::Bytes, extract::Extension, Json};
use hyper::StatusCode;
use lib_common::time::Utc;
use std::cmp::Ordering;

/// ADSB entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_ADSB: u32 = 10000;

/// CPR lat/lon entries in the cache will expire after 1 second
const CACHE_EXPIRE_MS_AIRCRAFT_CPR: u32 = 1000;

/// Number of times a packet must be received
///  from unique senders before it is considered valid
const N_REPORTERS_NEEDED: u32 = 1;

/// Data structure of encoded position data
struct GisPositionData {
    icao: u32,
    lat_cpr: u32,
    lon_cpr: u32,
    alt: u16,
    odd_flag: CPRFormat,
}

/// Data structure of encoded velocity data
struct GisVelocityData {
    icao: u32,
    st: u8,
    ew_sign: Sign,
    ew_vel: u16,
    ns_sign: Sign,
    ns_vel: u16,
    // vrate_src: VerticalRateSource,
    vrate_sign: Sign,
    vrate_value: u16,
    // gnss_sign: Sign,
    // gnss_baro_diff: u16,
}

// Decode aircraft type from ADS-B message type coding and aircraft category
fn get_aircraft_type(type_coding: TypeCoding, aircraft_category: u8) -> AircraftType {
    // in type coding
    // A = 4
    // B = 3
    // C = 2
    // D = 1
    match (type_coding, aircraft_category) {
        (TypeCoding::D, _) => AircraftType::Other,
        (_, 0) => AircraftType::Other,
        (TypeCoding::C, 1) => AircraftType::Other,
        (TypeCoding::C, 3) => AircraftType::Other,
        (TypeCoding::C, 4) => AircraftType::Groundobstacle,
        (TypeCoding::C, 5) => AircraftType::Groundobstacle,
        (TypeCoding::C, 6) => AircraftType::Groundobstacle,
        (TypeCoding::C, 7) => AircraftType::Groundobstacle,
        (TypeCoding::B, 1) => AircraftType::Glider,
        (TypeCoding::B, 2) => AircraftType::Airship,
        (TypeCoding::B, 3) => AircraftType::Unpowered,
        (TypeCoding::B, 4) => AircraftType::Glider,
        (TypeCoding::B, 5) => AircraftType::Other,
        // (TypeCoding::B, 6) => AircraftType::Uas, // Unmanned Aerial Vehicle
        (TypeCoding::B, 7) => AircraftType::Rocket,
        (TypeCoding::A, 7) => AircraftType::Rotorcraft,
        // TODO(R5): Support other types
        _ => AircraftType::Other,
    }
}

/// Pushes an aircraft identifier message to the queue
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) requires redis backend to test
async fn gis_identifier_push(
    identifier: String,
    type_coding: TypeCoding,
    aircraft_category: u8,
    mut gis_pool: GisPool,
) -> Result<(), ()> {
    let aircraft_type = get_aircraft_type(type_coding, aircraft_category);
    let item = AircraftId {
        identifier: Some(identifier),
        session_id: None,
        aircraft_type,
        timestamp_network: Utc::now(),
        timestamp_asset: None,
    };

    gis_pool
        .push::<AircraftId>(item, REDIS_KEY_AIRCRAFT_ID)
        .await
}

///
/// Pushes a position telemetry message to the queue
///
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) requires redis backend to test
async fn gis_position_push(
    data: GisPositionData,
    mut tlm_pool: TelemetryPool,
    mut gis_pool: GisPool,
) -> Result<(), ()> {
    if data.odd_flag == CPRFormat::Odd {
        rest_info!("received an odd flag CPR format message.");
        return Ok(()); // ignore even CPR format messages
    }

    // Get the even packet from the cache
    let keys = vec![
        format!("{:x}:lat_cpr:{}", data.icao, CPRFormat::Odd as u8),
        format!("{:x}:lon_cpr:{}", data.icao, CPRFormat::Odd as u8),
    ];

    let n_expected_results = keys.len();
    let results = tlm_pool.multiple_get::<u32>(keys).await.map_err(|e| {
        rest_warn!("could not get packet from cache: {e}");
    })?;

    if results.len() != n_expected_results {
        rest_warn!("unexpected result from cache.");
        return Err(());
    }

    let (e_lat_cpr, e_lon_cpr) = (results[0], results[1]);
    let (latitude, longitude) = decode_cpr(e_lat_cpr, e_lon_cpr, data.lat_cpr, data.lon_cpr)
        .map_err(|e| {
            rest_warn!("could not decode CPR: {e}");
        })?;

    let identifier = format!("{:x}", data.icao);
    let item = AircraftPosition {
        identifier: identifier.clone(),
        position: Position {
            latitude,
            longitude,
            altitude_meters: decode_altitude(data.alt) as f64,
        },
        timestamp_network: Utc::now(),
        timestamp_asset: None,
    };

    gis_pool
        .push::<AircraftPosition>(item, REDIS_KEY_AIRCRAFT_POSITION)
        .await
}

/// Pushes a velocity telemetry message to the queue
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) requires redis backend to test
async fn gis_velocity_push(data: GisVelocityData, mut gis_pool: GisPool) -> Result<(), ()> {
    let (velocity_horizontal_ground_mps, track_angle_degrees) = decode_speed_direction(
        data.st,
        data.ew_sign,
        data.ew_vel,
        data.ns_sign,
        data.ns_vel,
    )
    .map_err(|e| {
        rest_info!("could not decode speed and direction: {e}");
    })?;

    let velocity_vertical_mps =
        decode_vertical_speed(data.vrate_sign, data.vrate_value).map_err(|e| {
            rest_info!("could not decode vertical speed: {e}");
        })?;

    let item = AircraftVelocity {
        identifier: format!("{:x}", data.icao),
        velocity_horizontal_ground_mps,
        velocity_horizontal_air_mps: None,
        velocity_vertical_mps,
        track_angle_degrees,
        timestamp_asset: None,
        timestamp_network: Utc::now(),
    };

    gis_pool
        .push::<AircraftVelocity>(item, REDIS_KEY_AIRCRAFT_VELOCITY)
        .await
}

/// Post ADS-B Telemetry
/// Min 8 bytes, max 263 bytes
#[utoipa::path(
    post,
    path = "/telemetry/adsb",
    tag = "svc-telemetry",
    request_body = Vec<u8>,
    responses(
        (status = 200, description = "Telemetry received."),
        (status = 400, description = "Malformed packet."),
        (status = 500, description = "Something went wrong."),
        (status = 503, description = "Dependencies of svc-telemetry were down."),
    )
)]
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) requires redis backend to test
pub async fn adsb(
    Extension(mut tlm_pools): Extension<TelemetryPools>,
    Extension(gis_pool): Extension<GisPool>,
    Extension(mq_channel): Extension<lapin::Channel>,
    Extension(grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("entry.");
    //
    // ADS-B messages are 14 bytes long, small enough for a unique key
    // If the key is not in the cache, add it
    // If the key is in the cache, increment the count
    //
    let payload = <[u8; ADSB_SIZE_BYTES]>::try_from(payload.as_ref()).map_err(|_| {
        rest_error!("received ads-b message not {ADSB_SIZE_BYTES} bytes.");
        StatusCode::BAD_REQUEST
    })?;

    let key = crate::cache::bytes_to_key(&payload);
    let count = tlm_pools
        .adsb
        .increment(&key, CACHE_EXPIRE_MS_ADSB)
        .await
        .map_err(|e| {
            rest_error!("{e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match count.cmp(&N_REPORTERS_NEEDED) {
        Ordering::Less => {
            rest_error!("ADS-B reporter count should be impossible: {count}.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Greater => {
            rest_info!("ADS-B reporter count is greater than needed: {count}.");

            // TODO(R5) push up to N reporter confirmations to svc-storage with user_ids
            return Ok(Json(count));
        }
        _ => (), // continue
    }

    //
    // Deconstruct Packet
    //
    let frame = adsb_deku::Frame::from_bytes((&payload, 0)).map_err(|e| {
        rest_info!("could not parse ads-b message: {e}");
        StatusCode::BAD_REQUEST
    })?;

    let frame = frame.1;
    let adsb_deku::DF::ADSB(msg) = &frame.df else {
        rest_info!("received a non-ADSB format message.");
        return Err(StatusCode::BAD_REQUEST);
    };

    //
    // Get an identifiable key from the packet
    // Use the following keys to form a unique key per packet
    //  - ICAO address
    //  - odd/even flag
    //
    // Keys will expire automatically in the cache after some time.
    // The odd/even flag is used to differentiate between two packets
    //  that are part of the same message.
    let icao = get_adsb_icao_address(&msg.icao.0);

    match &msg.me {
        Identification(adsb_deku::adsb::Identification { tc, ca, cn }) => {
            gis_identifier_push(cn.clone(), *tc, *ca, gis_pool)
                .await
                .map_err(|_| {
                    rest_error!("could not push position to queue.");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            rest_info!("pushed position to queue.");
        }
        AirbornePosition(adsb_deku::Altitude {
            odd_flag,
            lat_cpr,
            lon_cpr,
            alt,
            ..
        }) => {
            let alt = alt.ok_or_else(|| {
                rest_info!("no altitude in packet.");
                StatusCode::BAD_REQUEST
            })?;

            let keyvals = vec![
                (
                    format!("{:x}:lat_cpr:{}", icao, odd_flag),
                    lat_cpr.to_string(),
                ),
                (
                    format!("{:x}:lon_cpr:{}", icao, odd_flag),
                    lon_cpr.to_string(),
                ),
            ];

            tlm_pools
                .adsb
                .multiple_set(keyvals, CACHE_EXPIRE_MS_AIRCRAFT_CPR)
                .await
                .map_err(|e| {
                    rest_error!("could not add lat/lon to cache: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            rest_info!("added lat/lon to cache.");

            let data = GisPositionData {
                icao,
                lat_cpr: *lat_cpr,
                lon_cpr: *lon_cpr,
                alt,
                odd_flag: *odd_flag,
            };

            gis_position_push(data, tlm_pools.adsb, gis_pool)
                .await
                .map_err(|_| {
                    rest_error!("could not push position to queue.");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            rest_info!("pushed position to queue.");
        }
        Velocity(adsb_deku::adsb::AirborneVelocity {
            st,
            sub_type,
            // vrate_src,
            vrate_sign,
            vrate_value,
            // gnss_sign,
            // gnss_baro_diff,
            ..
        }) => {
            // TODO(R5): Add navigation uncertainty field
            let AirborneVelocitySubType::GroundSpeedDecoding(GroundSpeedDecoding {
                ew_sign,
                ew_vel,
                ns_sign,
                ns_vel,
            }) = sub_type
            else {
                rest_info!("no ground speed in packet.");
                return Err(StatusCode::NOT_IMPLEMENTED);
            };

            let data = GisVelocityData {
                icao,
                st: *st,
                ew_sign: *ew_sign,
                ew_vel: *ew_vel,
                ns_sign: *ns_sign,
                ns_vel: *ns_vel,
                // vrate_src: *vrate_src,
                vrate_sign: *vrate_sign,
                vrate_value: *vrate_value,
                // gnss_sign: *gnss_sign,
                // gnss_baro_diff: *gnss_baro_diff,
            };

            gis_velocity_push(data, gis_pool).await.map_err(|_| {
                rest_error!("could not push velocity to queue.");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            rest_info!("pushed velocity to queue.");
        }
        _ => {
            // for now, reject non-position messages
            rest_info!("received an unrecognized message.");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    //
    // Send Telemetry to RabbitMQ
    //
    let _ = mq_channel
        .basic_publish(
            crate::amqp::EXCHANGE_NAME_TELEMETRY,
            crate::amqp::ROUTING_KEY_ADSB,
            lapin::options::BasicPublishOptions::default(),
            &payload,
            lapin::BasicProperties::default(),
        )
        .await
        .map_err(|e| rest_error!("telemetry push to RabbitMQ failed: {e}."))
        .map(|_| rest_info!("telemetry pushed to RabbitMQ."));

    //
    // Send to svc-storage
    //
    let data = adsb::Data {
        icao_address: icao as i64,
        message_type: get_adsb_message_type(&payload),
        network_timestamp: Some(Utc::now().into()),
        payload: payload.to_vec(),
    };

    // Make request
    let request = data;
    let client = &grpc_clients.storage.adsb;

    client.insert(request).await.map_err(|e| {
        rest_error!("telemetry push to svc-storage failed: {}.", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    rest_info!("telemetry pushed to svc-storage.");

    Ok(Json(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_aircraft_type() {
        // in type coding (TC)
        // A = 4
        // B = 3
        // C = 2
        // D = 1

        // TC = 1 (D) and any category is a reserved field
        assert_eq!(
            get_aircraft_type(TypeCoding::D, rand::random::<u8>()),
            AircraftType::Other
        );

        // TC = 2 (C) and category 1,3 are surface vehicles
        assert_eq!(get_aircraft_type(TypeCoding::C, 1), AircraftType::Other);
        assert_eq!(get_aircraft_type(TypeCoding::C, 3), AircraftType::Other);

        // TC = 2 (C) and category 4,5,6,7 are ground obstacles
        assert_eq!(
            get_aircraft_type(TypeCoding::C, 4),
            AircraftType::Groundobstacle
        );
        assert_eq!(
            get_aircraft_type(TypeCoding::C, 5),
            AircraftType::Groundobstacle
        );
        assert_eq!(
            get_aircraft_type(TypeCoding::C, 6),
            AircraftType::Groundobstacle
        );
        assert_eq!(
            get_aircraft_type(TypeCoding::C, 7),
            AircraftType::Groundobstacle
        );

        // TC = 3 (B) and category 1,4 are gliders
        assert_eq!(get_aircraft_type(TypeCoding::B, 1), AircraftType::Glider);
        assert_eq!(get_aircraft_type(TypeCoding::B, 4), AircraftType::Glider);

        // TC = 3 (B) and category 2 is a lighter than air aircraft
        assert_eq!(get_aircraft_type(TypeCoding::B, 2), AircraftType::Airship);

        // TC = 3 (B) and category 3 is a parachute/unpowered
        assert_eq!(get_aircraft_type(TypeCoding::B, 3), AircraftType::Unpowered);

        // TC = 3 (B) and category 5 is a reserved field
        assert_eq!(get_aircraft_type(TypeCoding::B, 5), AircraftType::Other);

        // TC = 3 (B) and category 6 is a UAV
        // assert_eq!(get_aircraft_type(TypeCoding::B, 6), AircraftType::Other);

        // TC = 3 (B) and category 7 is a rocket
        assert_eq!(get_aircraft_type(TypeCoding::B, 7), AircraftType::Rocket);

        // TC = 4 (A) and category 7 is a rotorcraft
        assert_eq!(
            get_aircraft_type(TypeCoding::A, 7),
            AircraftType::Rotorcraft
        );

        // everything else is 'other' for now
    }
}
