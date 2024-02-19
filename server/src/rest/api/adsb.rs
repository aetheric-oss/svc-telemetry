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
use chrono::Utc;
use hyper::StatusCode;
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

/// Pushes an aircraft identifier message to the queue
async fn gis_identifier_push(
    identifier: String,
    type_coding: TypeCoding,
    aircraft_category: u8,
    mut gis_pool: GisPool,
) -> Result<(), ()> {
    let aircraft_type: AircraftType = match (type_coding, aircraft_category) {
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
        (TypeCoding::B, 7) => AircraftType::Rocket,
        (TypeCoding::A, 7) => AircraftType::Rotorcraft,
        // TODO(R5): Support other types
        _ => AircraftType::Other,
    };

    let item = AircraftId {
        identifier,
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
async fn gis_position_push(
    data: GisPositionData,
    mut tlm_pool: TelemetryPool,
    mut gis_pool: GisPool,
) -> Result<(), ()> {
    if data.odd_flag == CPRFormat::Odd {
        rest_info!("(gis_position_push) received an odd flag CPR format message.");
        return Ok(()); // ignore even CPR format messages
    }

    // Get the even packet from the cache
    let keys = vec![
        format!("{:x}:lat_cpr:{}", data.icao, CPRFormat::Odd as u8),
        format!("{:x}:lon_cpr:{}", data.icao, CPRFormat::Odd as u8),
    ];

    let n_expected_results = keys.len();
    let Ok(results) = tlm_pool.multiple_get::<u32>(keys).await else {
        rest_warn!("(gis_position_push) could not get packet from cache.");
        return Err(());
    };

    if results.len() != n_expected_results {
        rest_warn!("(gis_position_push) unexpected result from cache.");
        return Err(());
    }

    let (e_lat_cpr, e_lon_cpr) = (results[0], results[1]);

    let Ok((latitude, longitude)) = decode_cpr(e_lat_cpr, e_lon_cpr, data.lat_cpr, data.lon_cpr)
    else {
        rest_warn!("(gis_position_push) could not decode CPR.");
        return Err(());
    };

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
async fn gis_velocity_push(data: GisVelocityData, mut gis_pool: GisPool) -> Result<(), ()> {
    let Ok((velocity_horizontal_ground_mps, track_angle_degrees)) = decode_speed_direction(
        data.st,
        data.ew_sign,
        data.ew_vel,
        data.ns_sign,
        data.ns_vel,
    ) else {
        rest_info!("(adsb) could not decode speed and direction.");
        return Err(());
    };

    let Ok(velocity_vertical_mps) = decode_vertical_speed(data.vrate_sign, data.vrate_value) else {
        rest_info!("(adsb) could not decode vertical speed.");
        return Err(());
    };

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
pub async fn adsb(
    Extension(mut tlm_pools): Extension<TelemetryPools>,
    Extension(gis_pool): Extension<GisPool>,
    Extension(mq_channel): Extension<lapin::Channel>,
    Extension(grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(adsb) entry.");
    //
    // ADS-B messages are 14 bytes long, small enough for a unique key
    // If the key is not in the cache, add it
    // If the key is in the cache, increment the count
    //
    let payload = <[u8; ADSB_SIZE_BYTES]>::try_from(payload.as_ref()).map_err(|_| {
        rest_error!("(adsb) received ads-b message not {ADSB_SIZE_BYTES} bytes.");
        StatusCode::BAD_REQUEST
    })?;

    let key = crate::cache::bytes_to_key(&payload);
    let count = tlm_pools
        .adsb
        .increment(&key, CACHE_EXPIRE_MS_ADSB)
        .await
        .map_err(|e| {
            rest_error!("(adsb) {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match count.cmp(&N_REPORTERS_NEEDED) {
        Ordering::Less => {
            rest_error!("(adsb) ADS-B reporter count should be impossible: {count}.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Greater => {
            rest_info!("(adsb) ADS-B reporter count is greater than needed: {count}.");

            // TODO(R4) push up to N reporter confirmations to svc-storage with user_ids
            return Ok(Json(count));
        }
        _ => (), // continue
    }

    //
    // Deconstruct Packet
    //
    let frame = adsb_deku::Frame::from_bytes((&payload, 0)).map_err(|e| {
        rest_info!("(adsb) could not parse ads-b message: {e}");
        StatusCode::BAD_REQUEST
    })?;

    let frame = frame.1;
    let adsb_deku::DF::ADSB(msg) = &frame.df else {
        rest_info!("(adsb) received a non-ADSB format message.");
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
            match gis_identifier_push(cn.clone(), *tc, *ca, gis_pool).await {
                Ok(_) => rest_info!("(adsb) pushed position to queue."),
                Err(_) => {
                    rest_error!("(adsb) could not push position to queue.");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
        AirbornePosition(adsb_deku::Altitude {
            odd_flag,
            lat_cpr,
            lon_cpr,
            alt,
            ..
        }) => {
            let Some(alt) = alt else {
                rest_info!("(adsb) no altitude in packet.");
                return Err(StatusCode::BAD_REQUEST);
            };

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

            match tlm_pools
                .adsb
                .multiple_set(keyvals, CACHE_EXPIRE_MS_AIRCRAFT_CPR)
                .await
            {
                Ok(_) => rest_info!("(adsb) added lat/lon to cache."),
                Err(e) => {
                    rest_error!("(adsb) could not add lat/lon to cache: {}.", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }

            let data = GisPositionData {
                icao,
                lat_cpr: *lat_cpr,
                lon_cpr: *lon_cpr,
                alt: *alt,
                odd_flag: *odd_flag,
            };

            match gis_position_push(data, tlm_pools.adsb, gis_pool).await {
                Ok(_) => rest_info!("(adsb) pushed position to queue."),
                Err(_) => {
                    rest_error!("(adsb) could not push position to queue.");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
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
                rest_info!("(adsb) no ground speed in packet.");
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

            match gis_velocity_push(data, gis_pool).await {
                Ok(_) => rest_info!("(adsb) pushed velocity to queue."),
                Err(_) => {
                    rest_error!("(adsb) could not push velocity to queue.");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
        _ => {
            // for now, reject non-position messages
            rest_info!("(adsb) received an unrecognized message.");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    //
    // Send Telemetry to RabbitMQ
    //
    let result = mq_channel
        .basic_publish(
            crate::amqp::EXCHANGE_NAME_TELEMETRY,
            crate::amqp::ROUTING_KEY_ADSB,
            lapin::options::BasicPublishOptions::default(),
            &payload,
            lapin::BasicProperties::default(),
        )
        .await;

    match result {
        Ok(_) => rest_info!("(adsb) telemetry pushed to RabbitMQ."),
        Err(e) => rest_error!("(adsb) telemetry push to RabbitMQ failed: {e}."),
    }

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

    match client.insert(request).await {
        Ok(_) => rest_info!("(adsb) telemetry pushed to svc-storage."),
        Err(e) => {
            rest_error!("(adsb) telemetry push to svc-storage failed: {}.", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(count))
}
