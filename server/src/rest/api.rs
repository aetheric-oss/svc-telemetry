//! REST API implementations for svc-telemetry

use crate::cache::pool::RedisPool;
use crate::grpc::client::GrpcClients;

use adsb_deku::deku::DekuContainerRead;
use axum::{body::Bytes, extract::Extension, Json};
use hyper::StatusCode;
use snafu::prelude::Snafu;
use std::cmp::Ordering;
use std::time::SystemTime;
use svc_storage_client_grpc::adsb;

/// Types Used in REST Messages
pub mod rest_types {
    include!("../../../openapi/types.rs");
}

pub use mavlink::{common::MavMessage, MavFrame, MavlinkVersion, Message};
pub use rest_types::Keys;

const ADSB_SIZE_BYTES: usize = 14;

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
    rest_info!("(process_mavlink) entry.");

    let Ok(frame) = MavFrame::<MavMessage>::deser(MavlinkVersion::V2, payload) else {
        return Err(ProcessError::CouldNotParse);
    };

    let key: u32 = frame.header().hashed_key();

    // Set the key
    let result = cache.try_key(key).await;
    let Ok(count) = result else {
        rest_error!("(process_mavlink) {}.", result.unwrap_err());
        return Err(ProcessError::CouldNotWriteCache);
    };

    Ok((frame, count))
}

/// Health check for load balancing
#[utoipa::path(
    get,
    path = "/health",
    tag = "svc-telemetry",
    responses(
        (status = 200, description = "Service is healthy, all dependencies running."),
        (status = 503, description = "Service is unhealthy, one or more dependencies unavailable.")
    )
)]
pub async fn health_check(
    Extension(mut grpc_clients): Extension<GrpcClients>,
) -> Result<(), StatusCode> {
    rest_info!("(health_check) entry.");

    let mut ok = true;

    let result = grpc_clients.adsb.get_client().await;
    if result.is_none() {
        let error_msg = "svc-storage unavailable.".to_string();
        rest_error!("(health_check) {}.", &error_msg);
        ok = false;
    };

    match ok {
        true => {
            rest_info!("(health_check) healthy, all dependencies running.");
            Ok(())
        }
        false => {
            rest_error!("(health_check) unhealthy, 1+ dependencies down.");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
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
    Extension(_ac): Extension<RedisPool>,
    Extension(mut _grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<i64>, StatusCode> {
    rest_info!("(mavlink_adsb) entry.");

    let result = process_mavlink(&payload, mavlink_cache).await;
    let Ok((_frame, count)) = result else {
        match result {
            Err(ProcessError::CouldNotParse) => {
                return Err(StatusCode::BAD_REQUEST);
            },
            _ => {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
    };

    match count.cmp(&1) {
        Ordering::Less => {
            rest_error!(
                "(mavlink_adsb) ADS-B report count should be impossible: {}.",
                count
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Equal => {
            rest_info!("(mavlink_adsb) first time this ADS-B packet was received.");
            // write raw packet to svc-storage
        }
        _ => {
            rest_info!(
                "(mavlink_adsb) confirmations received for this ADS-B packet: {}.",
                count
            );
            // increment confirmations to svc-storage?
        }
    };

    // Push to svc-storage
    // Push to third-party
    rest_info!("(mavlink_adsb) success");
    Ok(Json(count))
}

/// Parses the ADS-B packet for the message type filed
/// Bits 32-37 (0-index)
fn get_adsb_message_type(bytes: &[u8; ADSB_SIZE_BYTES]) -> i64 {
    // First 5 bits of the fifth byte
    ((bytes[4] >> 3) & 0x1F) as i64
}

/// Parses an ADS-B packet from bytes and reports the number of times
///  this specific packet has been received
async fn process_adsb(
    payload: &[u8],
    mut cache: RedisPool,
) -> Result<(i64, i64, i64), ProcessError> {
    rest_info!("(process_adsb) entry.");

    let Ok(payload) = <[u8; ADSB_SIZE_BYTES]>::try_from(payload) else {
        rest_info!("(process_adsb) received ads-b message not {ADSB_SIZE_BYTES} bytes.");
        return Err(ProcessError::CouldNotParse);
    };

    let Ok(frame) = adsb_deku::Frame::from_bytes((&payload, 0)) else {
        rest_info!("(process_adsb) could not parse ads-b message.");
        return Err(ProcessError::CouldNotParse);
    };

    let frame = frame.1;
    let adsb_deku::DF::ADSB(_) = &frame.df else {
        rest_info!("(process_adsb) received a non-ADSB format message.");
        return Err(ProcessError::CouldNotParse);
    };

    let key: u32 = frame.hashed_key();

    // Set the key
    let result = cache.try_key(key).await;
    let Ok(count) = result else {
        rest_error!("(process_adsb) {}", result.unwrap_err());
        return Err(ProcessError::CouldNotWriteCache);
    };

    Ok((
        frame.primary_key() as i64,
        get_adsb_message_type(&payload),
        count,
    ))
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
pub async fn adsb(
    Extension(_mc): Extension<RedisPool>,
    Extension(adsb_cache): Extension<RedisPool>,
    Extension(mut grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<i64>, StatusCode> {
    rest_info!("(adsb) entry.");

    let result = process_adsb(&payload, adsb_cache).await;
    let Ok((icao_address, message_type, count)) = result else {
        match result {
            Err(ProcessError::CouldNotParse) => {
                return Err(StatusCode::BAD_REQUEST);
            },
            _ => {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
    };

    match count.cmp(&1) {
        Ordering::Less => {
            rest_info!("(adsb) ADS-B report count should be impossible: {}.", count);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Equal => {
            // continue
        }
        _ => {
            rest_info!(
                "(adsb) confirmations received for this ADS-B packet: {}.",
                count
            );

            // increment confirmations to svc-storage when crowdsourcing telemetry

            return Ok(Json(count));
        }
    };

    rest_info!("(adsb) first time this ADS-B packet was received.");

    let current_time = prost_types::Timestamp::from(SystemTime::now());
    let data = adsb::Data {
        icao_address,
        message_type,
        network_timestamp: Some(current_time),
        payload: payload.to_vec(),
    };

    // Make request
    let request = tonic::Request::new(data);
    let Some(mut client) = grpc_clients.adsb.get_client().await else {
        rest_error!("(adsb) could not get svc-storage client.");
        grpc_clients.adsb.invalidate().await;
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    let response = client.insert(request).await;
    if response.is_err() {
        rest_error!("(adsb) telemetry push to svc-storage failed.");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    rest_info!("(adsb) success.");
    Ok(Json(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_get_adsb_message_type() -> Result<(), Box<dyn std::error::Error>> {
        {
            let expected_message_type: i64 = 4;
            let payload: [u8; 14] = [
                0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
            ];
            println!("{}", payload.len());
            let Ok(bytes) = <[u8; ADSB_SIZE_BYTES]>::try_from(payload) else { panic!(); };
            assert_eq!(get_adsb_message_type(&bytes), expected_message_type);
        }

        {
            let expected_message_type: i64 = 11;
            let payload: [u8; 14] = [
                0x8D, 0x40, 0x62, 0x1D, 0x58, 0xC3, 0x82, 0xD6, 0x90, 0xC8, 0xAC, 0x28, 0x63, 0xA7,
            ];
            println!("{}", payload.len());
            let Ok(bytes) = <[u8; ADSB_SIZE_BYTES]>::try_from(payload) else { panic!(); };
            assert_eq!(get_adsb_message_type(&bytes), expected_message_type);
        }

        {
            let expected_message_type: i64 = 19;
            let payload: [u8; 14] = [
                0x8D, 0x48, 0x50, 0x20, 0x99, 0x44, 0x09, 0x94, 0x08, 0x38, 0x17, 0x5B, 0x28, 0x4F,
            ];
            println!("{}", payload.len());
            let Ok(bytes) = <[u8; ADSB_SIZE_BYTES]>::try_from(payload) else { panic!(); };
            assert_eq!(get_adsb_message_type(&bytes), expected_message_type);
        }

        Ok(())
    }
}
