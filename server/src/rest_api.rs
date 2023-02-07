//! REST API implementations for svc-telemetry

use crate::cache::RedisPool;
use crate::grpc_clients::GrpcClients;
use axum::{body::Bytes, extract::Extension, Json};
use snafu::prelude::Snafu;
use std::cmp::Ordering;

/// Types Used in REST Messages
pub mod rest_types {
    include!("../../openapi/types.rs");
}

pub use mavlink::{common::MavMessage, MavFrame, MavlinkVersion, Message};

pub use rest_types::Keys;

// /// Writes an info! message to the app::req logger
// macro_rules! req_info {
//     ($($arg:tt)+) => {
//         log::info!(target: "app::req", $($arg)+);
//     };
// }

// /// Writes an error! message to the app::req logger
macro_rules! req_error {
    ($($arg:tt)+) => {
        log::error!(target: "app::req", $($arg)+);
    };
}

/// Writes a debug! message to the app::req logger
macro_rules! req_debug {
    ($($arg:tt)+) => {
        log::debug!(target: "app::req", $($arg)+);
    };
}

#[derive(Debug, Snafu)]
enum ProcessError {
    #[snafu(display("Could not parse the packet."))]
    CouldNotParse,

    #[snafu(display("Could not write to the cache."))]
    CouldNotWriteCache,
}

/// Parses a Mavlink packet from bytes and reports the number of times
///  this specific packet has been received
async fn process_mavlink(
    payload: &[u8],
    mut cache: RedisPool,
) -> Result<(MavFrame<MavMessage>, i64), ProcessError> {
    req_debug!("(process_mavlink) entry");

    let Ok(frame) = MavFrame::<MavMessage>::deser(MavlinkVersion::V2, payload) else {
        return Err(ProcessError::CouldNotParse);
    };

    let key: u32 = frame.header().hashed_key();

    // Set the key
    let result = cache.try_key(key).await;
    let Ok(count) = result else {
        req_error!("{}", result.unwrap_err());
        return Err(ProcessError::CouldNotWriteCache);
    };

    Ok((frame, count))
}

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
    Extension(mavlink_cache): Extension<RedisPool>,
    Extension(mut _grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<i64>, hyper::StatusCode> {
    req_debug!("(mavlink_adsb) entry");

    let result = process_mavlink(&payload, mavlink_cache).await;
    let Ok((_frame, count)) = result else {
        match result {
            Err(ProcessError::CouldNotParse) => {
                return Err(hyper::StatusCode::BAD_REQUEST);
            },
            _ => {
                return Err(hyper::StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
    };

    match count.cmp(&1) {
        Ordering::Less => {
            req_debug!(
                "(mavlink_adsb) ADS-B report count should be impossible: {}.",
                count
            );
            return Err(hyper::StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Equal => {
            req_debug!("(mavlink_adsb) first time this ADS-B packet was received.");
            // write raw packet to svc-storage
        }
        _ => {
            req_debug!(
                "(mavlink_adsb) confirmations received for this ADS-B packet: {}",
                count
            );
            // increment confirmations to svc-storage?
        }
    };

    // Push to svc-storage
    // Push to third-party
    req_debug!("(mavlink_adsb) success");
    Ok(Json(count))
}
