//! REST API implementations for svc-telemetry

use crate::amqp::AMQPChannel;
use crate::cache::pool::RedisPool;
use crate::cache::RedisPools;
use crate::grpc::client::GrpcClients;
use adsb_deku::{deku::DekuContainerRead, Frame};
use axum::{body::Bytes, extract::Extension, Json};
use hyper::StatusCode;
use snafu::prelude::Snafu;
use std::cmp::Ordering;
use std::time::SystemTime;
use svc_storage_client_grpc::{adsb, ClientConnect, SimpleClient};

/// Types Used in REST Messages
pub mod rest_types {
    include!("../../../openapi/types.rs");
}

pub use mavlink::{common::MavMessage, MavFrame, MavlinkVersion, Message};

pub use rest_types::Keys;

/// Expected size of ADSB packets
const ADSB_SIZE_BYTES: usize = 14;

/// Number of times a packet must be received before it is considered valid
/// TODO(R4): Raise after implementing unique confirmations based on user_id
const N_REPORTERS_NEEDED: u32 = 1;

/// ADSB Message Metadata
#[derive(Debug, Clone, Copy)]
pub struct HeaderData {
    /// Header ICAO address
    pub icao_address: i64,
    /// Header Message Type
    pub message_type: i64,
}

#[derive(Debug, Snafu)]
enum ProcessError {
    #[snafu(display("Could not parse the packet."))]
    CouldNotParse,

    #[snafu(display("Could not write to the cache."))]
    CouldNotWriteCache,

    #[snafu(display("Could not write to queue."))]
    CouldNotWriteMQ,
}

///  try packet should only be forwarded once, and only after it has
///  been received [`N_REPORTERS_NEEDED`] times by unique authorized
///  users.
async fn handle_adsb(
    payload: &[u8],
    header_data: HeaderData,
    reporter_count: u32,
    mq_channel: AMQPChannel,
    grpc_clients: GrpcClients,
) -> Result<Json<u32>, StatusCode> {
    rest_info!(
        "(handle_adsb) icao={}, type={}, reporter={}.",
        header_data.icao_address,
        header_data.message_type,
        reporter_count
    );

    match reporter_count.cmp(&N_REPORTERS_NEEDED) {
        Ordering::Less => {
            rest_error!(
                "(handle_adsb) ADS-B reporter count should be impossible: {}.",
                reporter_count
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Greater => {
            rest_info!(
                "(handle_adsb) ADS-B reporter count is greater than needed: {}.",
                reporter_count
            );
            // TODO(R4) push up to N reporter confirmations to svc-storage with user_ids
            return Ok(Json(reporter_count));
        }
        _ => (), // continue
    }

    // Send to RabbitMQ
    let result = mq_channel
        .basic_publish(
            crate::amqp::EXCHANGE_NAME_TELEMETRY,
            crate::amqp::ROUTING_KEY_ADSB,
            lapin::options::BasicPublishOptions::default(),
            payload,
            lapin::BasicProperties::default(),
        )
        .await;

    match result {
        Ok(_) => rest_info!("(handle_adsb) telemetry pushed to RabbitMQ."),
        Err(e) => rest_error!("(handle_adsb) telemetry push to RabbitMQ failed: {}.", e),
    }

    // Send to svc-storage
    let current_time = prost_types::Timestamp::from(SystemTime::now());
    let data = adsb::Data {
        icao_address: header_data.icao_address,
        message_type: header_data.message_type,
        network_timestamp: Some(current_time),
        payload: payload.to_vec(),
    };

    // Make request
    let request = tonic::Request::new(data);
    let client = &grpc_clients.storage.adsb;

    match client.insert(request).await {
        Ok(_) => rest_info!("(handle_adsb) telemetry pushed to svc-storage."),
        Err(e) => {
            rest_error!("(handle_adsb) telemetry push to svc-storage failed: {}.", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(reporter_count))
}

/// Parses a Mavlink packet from bytes and reports the number of times
///  this specific packet has been received
async fn process_mavlink(
    payload: &[u8],
    mut cache: RedisPool,
) -> Result<(HeaderData, u32), ProcessError> {
    rest_info!("(process_mavlink) entry.");

    let Ok(frame) = MavFrame::<MavMessage>::deser(MavlinkVersion::V2, payload) else {
        return Err(ProcessError::CouldNotParse);
    };

    let MavMessage::ADSB_VEHICLE(adsb) = frame.msg.clone() else {
        rest_info!("(process_mavlink) Could not parse mavlink message into ADSB_VEHICLE_DATA.");
        return Err(ProcessError::CouldNotParse);
    };

    let key: u32 = frame.header().hashed_key();

    // Set the key
    let result = cache.try_key(key).await;
    let Ok(count) = result else {
        rest_error!("(process_mavlink) {}.", result.unwrap_err());
        return Err(ProcessError::CouldNotWriteCache);
    };

    let header = HeaderData {
        icao_address: adsb.ICAO_address as i64,
        message_type: -1, // TODO(R3) Mavlink doesn't have traditional ADSB message types
    };

    Ok((header, count))
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
    Extension(grpc_clients): Extension<GrpcClients>,
) -> Result<(), StatusCode> {
    rest_debug!("(health_check) entry.");

    let mut ok = true;

    if grpc_clients.storage.adsb.get_client().await.is_err() {
        let error_msg = "svc-storage adsb unavailable.".to_string();
        rest_error!("(health_check) {}.", &error_msg);
        ok = false;
    }

    match ok {
        true => {
            rest_debug!("(health_check) healthy, all dependencies running.");
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
    Extension(pools): Extension<RedisPools>,
    Extension(mq_channel): Extension<lapin::Channel>,
    Extension(grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(mavlink_adsb) entry.");

    let result = process_mavlink(&payload, pools.mavlink).await;
    let Ok((header_data, count)) = result else {
        match result {
            Err(ProcessError::CouldNotParse) => {
                return Err(StatusCode::BAD_REQUEST);
            },
            _ => {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
    };

    handle_adsb(
        &payload,
        header_data,
        count,
        AMQPChannel {
            channel: Some(mq_channel),
        },
        grpc_clients,
    )
    .await
}

/// Parses the ADS-B packet for the message type filed
/// Bits 32-37 (0-index)
fn get_adsb_message_type(bytes: &[u8; ADSB_SIZE_BYTES]) -> i64 {
    // First 5 bits of the fifth byte
    ((bytes[4] >> 3) & 0x1F) as i64
}

/// Parses the message payload
/// Returns a new [`Frame`] and the correct payload size based on [`ADSB_SIZE_BYTES`]
fn get_frame_for_payload(payload: &[u8]) -> Result<(Frame, [u8; ADSB_SIZE_BYTES]), ProcessError> {
    rest_info!("(get_frame_for_payload) entry.");

    let Ok(payload) = <[u8; ADSB_SIZE_BYTES]>::try_from(payload) else {
        rest_info!("(get_frame_for_payload) received ads-b message not {ADSB_SIZE_BYTES} bytes.");
        return Err(ProcessError::CouldNotParse);
    };

    let Ok(frame) = adsb_deku::Frame::from_bytes((&payload, 0)) else {
        rest_info!("(get_frame_for_payload) could not parse ads-b message.");
        return Err(ProcessError::CouldNotParse);
    };

    let frame = frame.1;
    let adsb_deku::DF::ADSB(_) = &frame.df else {
        rest_info!("(get_frame_for_payload) received a non-ADSB format message.");
        return Err(ProcessError::CouldNotParse);
    };

    Ok((frame, payload))
}

/// Parses an ADS-B packet from bytes and reports the number of times
///  this specific packet has been received
async fn process_adsb(
    payload: &[u8],
    mut cache: RedisPool,
) -> Result<(HeaderData, u32), ProcessError> {
    let (frame, payload) = get_frame_for_payload(payload)?;
    let key: u32 = frame.hashed_key();

    // Set the key
    let result = cache.try_key(key).await;
    let Ok(count) = result else {
        rest_error!("(process_adsb) {}", result.unwrap_err());
        return Err(ProcessError::CouldNotWriteCache);
    };

    let header_data = HeaderData {
        icao_address: frame.primary_key() as i64,
        message_type: get_adsb_message_type(&payload),
    };

    Ok((header_data, count))
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
    Extension(pools): Extension<RedisPools>,
    Extension(mq_channel): Extension<lapin::Channel>,
    Extension(grpc_clients): Extension<GrpcClients>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(adsb) entry.");

    let result = process_adsb(&payload, pools.adsb).await;
    let Ok((header_data, count)) = result else {
        match result {
            Err(ProcessError::CouldNotParse) => {
                return Err(StatusCode::BAD_REQUEST);
            },
            _ => {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
    };

    handle_adsb(
        &payload,
        header_data,
        count,
        AMQPChannel {
            channel: Some(mq_channel),
        },
        grpc_clients,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

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

    #[tokio::test]
    async fn test_health_check_success() {
        // Mock the GrpcClients extension
        let config = Config::default();
        let grpc_clients = GrpcClients::default(config);

        // Call the health_check function
        let result = health_check(Extension(grpc_clients)).await;

        // Assert the expected result
        println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_adsb_svc_storage() {
        // Mock the GrpcClients extension
        let config = Config::default();
        let grpc_clients = GrpcClients::default(config);

        let payload: [u8; 14] = [
            0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
        ];

        let result = get_frame_for_payload(&payload);
        assert!(result.is_ok());
        let (frame, payload) = result.unwrap();
        let header_data = HeaderData {
            icao_address: frame.primary_key() as i64,
            message_type: get_adsb_message_type(&payload),
        };

        let result = handle_adsb(
            &payload,
            header_data,
            1,
            AMQPChannel { channel: None },
            grpc_clients,
        )
        .await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }
}
