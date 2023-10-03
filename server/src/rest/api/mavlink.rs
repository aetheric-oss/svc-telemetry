//! Mavlink REST API

pub use mavlink::{common::MavMessage, MavFrame, MavlinkVersion, Message};
// use crate::amqp::AMQPChannel;
// use crate::cache::pool::RedisPool;
use crate::cache::RedisPools;
use crate::grpc::client::GrpcClients;
use axum::{body::Bytes, extract::Extension, Json};
use hyper::StatusCode;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use svc_gis_client_grpc::client::AircraftPosition;

/// Maximum size of a mavlink packet
const MAVLINK_PKT_MAX_SIZE_BYTES: usize = 280;

/// Mavlink entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_MAVLINK_ADSB: u32 = 5000;

/// Number of times a packet must be received
///  from unique senders before it is considered valid
const N_REPORTERS_NEEDED: u32 = 1;

/// Post Mavlink Telemetry
/// Min 8 bytes, max 263 bytes
#[utoipa::path(
    post,
    path = "/telemetry/mavlink/adsb",
    tag = "svc-telemetry",
    request_body = Vec<u8>,
    responses(
        (status = 200, description = "Telemetry received."),
        (status = 400, description = "Malformed packet."),
        (status = 500, description = "Something went wrong."),
    )
)]
pub async fn mavlink_adsb(
    Extension(mut pools): Extension<RedisPools>,
    Extension(_mq_channel): Extension<lapin::Channel>,
    Extension(_grpc_clients): Extension<GrpcClients>,
    Extension(_ring): Extension<Arc<Mutex<VecDeque<AircraftPosition>>>>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(mavlink_adsb) entry.");

    if payload.len() > MAVLINK_PKT_MAX_SIZE_BYTES {
        rest_error!("(mavlink_adsb) packet too large: {} bytes.", payload.len());
        return Err(StatusCode::BAD_REQUEST);
    }

    let Ok(key) = std::str::from_utf8(&payload[..]) else {
        rest_error!("(mavlink_adsb) could not convert payload to string.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let result = pools
        .adsb
        .increment(key, CACHE_EXPIRE_MS_MAVLINK_ADSB)
        .await;
    let Ok(count) = result else {
        rest_error!("(mavlink_adsb) {}", result.unwrap_err());
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    match count.cmp(&N_REPORTERS_NEEDED) {
        Ordering::Less => {
            rest_error!("(mavlink_adsb) ADS-B reporter count should be impossible: {count}.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Greater => {
            rest_info!("(mavlink_adsb) ADS-B reporter count is greater than needed: {count}.");

            // TODO(R4) push up to N reporter confirmations to svc-storage with user_ids
            return Ok(Json(count));
        }
        _ => (), // continue
    }

    rest_info!("(mavlink_adsb) received first mavlink packet: {key}.");

    Ok(Json(count))
}
