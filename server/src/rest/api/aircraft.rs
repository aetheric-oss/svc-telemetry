//! Endpoints for updating aircraft positions

use crate::cache::pool::RedisPool;
use crate::cache::RedisPools;
use crate::grpc::client::GrpcClients;
use crate::msg::adsb::{
    decode_altitude, decode_cpr, get_adsb_icao_address, get_adsb_message_type, ADSB_SIZE_BYTES,
};
use adsb_deku::adsb::ME::AirbornePositionBaroAltitude as Position;
use adsb_deku::deku::DekuContainerRead;
use adsb_deku::CPRFormat;
use svc_gis_client_grpc::client::{AircraftPosition, AircraftType, Coordinates};
use svc_storage_client_grpc::prelude::*;
use svc_storage_client_grpc::resources::adsb;

use axum::{body::Bytes, extract::Extension, Json};
use chrono::Utc;
use hyper::StatusCode;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// ADSB entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_AIRCRAFT_ADSB: u32 = 10000;

/// CPR lat/lon entries in the cache will expire after 1 second
const CACHE_EXPIRE_MS_AIRCRAFT_CPR: u32 = 1000;

/// Number of times a packet must be received
///  from unique senders before it is considered valid
const N_REPORTERS_NEEDED: u32 = 1;

///
/// Pushes a position telemetry message to the ring buffer
///
pub async fn gis_position_push(
    icao: u32,
    lat_cpr: u32,
    lon_cpr: u32,
    alt: u16,
    odd_flag: CPRFormat,
    aircraft_type: AircraftType,
    mut pool: RedisPool,
    ring: Arc<Mutex<VecDeque<AircraftPosition>>>,
) -> Result<(), ()> {
    if odd_flag == CPRFormat::Odd {
        rest_info!("(gis_position_push) received an odd flag CPR format message.");
        return Ok(()); // ignore even CPR format messages
    }

    // Get the even packet from the cache
    let keys = vec![
        format!("{:x}:lat_cpr:{}", icao, CPRFormat::Odd as u8),
        format!("{:x}:lon_cpr:{}", icao, CPRFormat::Odd as u8),
    ];

    let n_expected_results = keys.len();
    let Ok(results) = pool.multiple_get::<u32>(keys).await else {
        rest_warn!("(gis_position_push) could not get packet from cache.");
        return Err(());
    };

    if results.len() != n_expected_results {
        rest_warn!("(gis_position_push) unexpected result from cache.");
        return Err(());
    }

    let (e_lat_cpr, e_lon_cpr) = (results[0], results[1]);

    let Ok((latitude, longitude)) = decode_cpr(e_lat_cpr, e_lon_cpr, lat_cpr, lon_cpr) else {
        rest_warn!("(gis_position_push) could not decode CPR.");
        return Err(());
    };

    let item = AircraftPosition {
        identifier: format!("{:x}", icao),
        location: Some(Coordinates {
            latitude,
            longitude,
        }),
        aircraft_type: aircraft_type as i32,
        altitude_meters: decode_altitude(alt),
        timestamp_network: Some(Utc::now().into()),
        timestamp_aircraft: None,
        uuid: None,
    };

    match ring.lock() {
        Ok(mut ring) => {
            rest_debug!(
                "(gis_position_push) pushing to ring buffer (items: {})",
                ring.len()
            );
            ring.push_back(item);
            Ok(())
        }
        _ => {
            rest_warn!("(gis_position_push) could not push to ring buffer.");
            Err(())
        }
    }
}

/// Post ADS-B Telemetry
/// Min 8 bytes, max 263 bytes
#[utoipa::path(
    post,
    path = "/telemetry/aircraft/adsb",
    tag = "svc-telemetry",
    request_body = Vec<u8>,
    responses(
        (status = 200, description = "Telemetry received."),
        (status = 400, description = "Malformed packet."),
        (status = 500, description = "Something went wrong."),
        (status = 503, description = "Dependencies of svc-telemetry were down."),
    )
)]
pub async fn aircraft_adsb(
    Extension(mut pools): Extension<RedisPools>,
    Extension(mq_channel): Extension<lapin::Channel>,
    Extension(grpc_clients): Extension<GrpcClients>,
    Extension(ring): Extension<Arc<Mutex<VecDeque<AircraftPosition>>>>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(aircraft_adsb) entry.");
    //
    // ADS-B messages are 14 bytes long, small enough for a unique key
    // If the key is not in the cache, add it
    // If the key is in the cache, increment the count
    //
    let Ok(key) = std::str::from_utf8(&payload[..]) else {
        rest_error!("(aircraft_adsb) could not convert payload to string.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let result = pools
        .adsb
        .increment(key, CACHE_EXPIRE_MS_AIRCRAFT_ADSB)
        .await;
    let Ok(count) = result else {
        rest_error!("(aircraft_adsb) {}", result.unwrap_err());
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    match count.cmp(&N_REPORTERS_NEEDED) {
        Ordering::Less => {
            rest_error!("(aircraft_adsb) ADS-B reporter count should be impossible: {count}.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Greater => {
            rest_info!("(aircraft_adsb) ADS-B reporter count is greater than needed: {count}.");

            // TODO(R4) push up to N reporter confirmations to svc-storage with user_ids
            return Ok(Json(count));
        }
        _ => (), // continue
    }

    //
    // Deconstruct Packet
    //
    let Ok(payload) = <[u8; ADSB_SIZE_BYTES]>::try_from(payload.as_ref()) else {
        rest_info!("(aircraft_adsb) received ads-b message not {ADSB_SIZE_BYTES} bytes.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let Ok(frame) = adsb_deku::Frame::from_bytes((&payload, 0)) else {
        rest_info!("(aircraft_adsb) could not parse ads-b message.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let frame = frame.1;
    let adsb_deku::DF::ADSB(msg) = &frame.df else {
        rest_info!("(aircraft_adsb) received a non-ADSB format message.");
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

    // TODO(R4): Get the aircraft type from wake vortex category of ADS-b
    // https://mode-s.org/decode/content/ads-b/2-identification.html
    let aircraft_type = AircraftType::Undeclared;
    match msg.me {
        Position(adsb_deku::Altitude {
            odd_flag,
            lat_cpr,
            lon_cpr,
            alt,
            ..
        }) => {
            let Some(alt) = alt else {
                rest_info!("(aircraft_adsb) no altitude in packet.");
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

            match pools
                .adsb
                .multiple_set(keyvals, CACHE_EXPIRE_MS_AIRCRAFT_CPR)
                .await
            {
                Ok(_) => rest_info!("(aircraft_adsb) added lat/lon to cache."),
                Err(e) => {
                    rest_error!("(aircraft_adsb) could not add lat/lon to cache: {}.", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }

            match gis_position_push(
                icao,
                lat_cpr,
                lon_cpr,
                alt,
                odd_flag,
                aircraft_type,
                pools.adsb,
                ring,
            )
            .await
            {
                Ok(_) => rest_info!("(aircraft_adsb) pushed position to ring buffer."),
                Err(_) => {
                    rest_error!("(aircraft_adsb) could not push position to ring buffer.");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
        // TODO(R4): Add Aircraft ID message here
        //  https://mode-s.org/decode/content/ads-b/2-identification.html
        //  Update GIS to indicate what type of aircraft the ICAO address is
        _ => {
            // for now, reject non-position messages
            rest_info!("(aircraft_adsb) received an unrecognized message.");
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
        Ok(_) => rest_info!("(aircraft_adsb) telemetry pushed to RabbitMQ."),
        Err(e) => rest_error!("(aircraft_adsb) telemetry push to RabbitMQ failed: {e}."),
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
        Ok(_) => rest_info!("(aircraft_adsb) telemetry pushed to svc-storage."),
        Err(e) => {
            rest_error!(
                "(aircraft_adsb) telemetry push to svc-storage failed: {}.",
                e
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(count))
}
