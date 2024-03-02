//! Remote ID REST API (Network Remote ID)
//!  Remote ID is a system for identifying and locating drones.
//!  It will be required for use of U-Space airspace by unmanned aircraft.
//! Endpoints for updating aircraft positions

use crate::cache::pool::GisPool;
use crate::cache::TelemetryPools;
use crate::msg::netrid::{
    BasicMessage, Frame, LocationMessage, MessageType, UaType as NetridAircraftType,
};
use svc_gis_client_grpc::prelude::types::*;

use axum::{body::Bytes, extract::Extension, Json};
use chrono::Utc;
use hyper::StatusCode;
use packed_struct::PackedStruct;
use std::cmp::Ordering;

/// Remote ID entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_NETRID: u32 = 10000;

/// Number of times a packet must be received
///  from unique senders before it is considered valid
const N_REPORTERS_NEEDED: u32 = 1;

/// Length of a remote id packet
const REMOTE_ID_PACKET_LENGTH: usize = 25;

impl From<NetridAircraftType> for AircraftType {
    fn from(t: NetridAircraftType) -> Self {
        match t {
            NetridAircraftType::Undeclared => AircraftType::Undeclared,
            NetridAircraftType::Aeroplane => AircraftType::Aeroplane,
            NetridAircraftType::Rotorcraft => AircraftType::Rotorcraft,
            NetridAircraftType::Gyroplane => AircraftType::Gyroplane,
            NetridAircraftType::HybridLift => AircraftType::Hybridlift,
            NetridAircraftType::Ornithopter => AircraftType::Ornithopter,
            NetridAircraftType::Glider => AircraftType::Glider,
            NetridAircraftType::Kite => AircraftType::Kite,
            NetridAircraftType::FreeBalloon => AircraftType::Freeballoon,
            NetridAircraftType::CaptiveBalloon => AircraftType::Captiveballoon,
            NetridAircraftType::Airship => AircraftType::Airship,
            NetridAircraftType::Unpowered => AircraftType::Unpowered,
            NetridAircraftType::Rocket => AircraftType::Rocket,
            NetridAircraftType::Tethered => AircraftType::Tethered,
            NetridAircraftType::GroundObstacle => AircraftType::Groundobstacle,
            NetridAircraftType::Other => AircraftType::Other,
        }
    }
}

/// Processes a basic remote id message type
async fn process_basic_message(
    _identifier: String,
    message: BasicMessage,
    mut gis_pool: GisPool,
    mq_channel: lapin::Channel,
) -> Result<(), StatusCode> {
    let aircraft_type = AircraftType::from(message.ua_type);

    // TODO(R5): Compare the identifier given for the JWT with the identifier in the message
    let Ok(identifier) = String::from_utf8(message.uas_id.to_vec()) else {
        rest_warn!("(process_basic_message) could not parse identifier to string.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let id_item = AircraftId {
        identifier,
        aircraft_type,
        timestamp_network: Utc::now(),
        timestamp_asset: None,
    };

    gis_pool
        .push::<AircraftId>(id_item.clone(), REDIS_KEY_AIRCRAFT_ID)
        .await
        .map_err(|_| {
            rest_warn!("(process_basic_message) could not push aircraft id to cache.");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    rest_debug!("(process_basic_message) pushed aircraft id to redis.");

    //
    // Send Telemetry to RabbitMQ
    //
    if let Ok(msg) = serde_json::to_vec(&id_item) {
        let _ = mq_channel
            .basic_publish(
                crate::amqp::EXCHANGE_NAME_TELEMETRY,
                crate::amqp::ROUTING_KEY_NETRID_ID,
                lapin::options::BasicPublishOptions::default(),
                &msg,
                lapin::BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                rest_warn!("(process_basic_message) could not push aircraft id to RabbitMQ: {e}.");
            });

        rest_debug!("(process_basic_message) pushed aircraft id to RabbitMQ.");
    } else {
        rest_warn!("(process_basic_message) could not serialize id item.");
    }

    Ok(())
}

/// Processes a basic remote id message type
async fn process_location_message(
    identifier: String,
    message: LocationMessage,
    mut gis_pool: GisPool,
    mq_channel: lapin::Channel,
) -> Result<(), StatusCode> {
    //
    // TODO(R5): Decide what to do when a field is UNKNOWN
    //  Reject the whole message? Use the 'unknown' value (e.g. 63.0 for vertical rate)?
    //  What if only one field fails validation and the rest don't?
    //

    let Ok(altitude_meters) = message.decode_altitude() else {
        rest_warn!("(process_basic_message) could not parse altitude.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let Ok(velocity_horizontal_ground_mps) = message.decode_speed() else {
        rest_warn!("(process_basic_message) could not parse speed.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let Ok(velocity_vertical_mps) = message.decode_vertical_speed() else {
        rest_warn!("(process_basic_message) could not parse vertical speed.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let timestamp_asset = match message.decode_timestamp() {
        Ok(ts) => Some(ts),
        Err(_) => None,
    };

    let latitude = message.decode_latitude();
    let longitude = message.decode_longitude();

    let position_item = AircraftPosition {
        identifier: identifier.clone(),
        position: Position {
            latitude,
            longitude,
            altitude_meters: altitude_meters as f64,
        },
        timestamp_network: Utc::now(),
        timestamp_asset,
    };

    let velocity_item = AircraftVelocity {
        identifier,
        velocity_vertical_mps,
        velocity_horizontal_ground_mps,
        velocity_horizontal_air_mps: None,
        track_angle_degrees: message.decode_direction() as f32,
        timestamp_asset,
        timestamp_network: Utc::now(),
    };

    gis_pool
        .push::<AircraftPosition>(position_item.clone(), REDIS_KEY_AIRCRAFT_POSITION)
        .await
        .map_err(|_| {
            rest_warn!("(process_basic_message) could not push aircraft position to cache.");
            StatusCode::INTERNAL_SERVER_ERROR
        })?; // TODO(R5): Do we want to bail here or still send the velocity to postgis?

    rest_debug!("(process_basic_message) pushed aircraft position to redis.");

    let _ = gis_pool
        .push::<AircraftVelocity>(velocity_item.clone(), REDIS_KEY_AIRCRAFT_VELOCITY)
        .await
        .map_err(|_| {
            rest_warn!("(process_basic_message) could not push aircraft velocity to cache.");
            // StatusCode::INTERNAL_SERVER_ERROR
        });

    rest_debug!("(process_basic_message) pushed aircraft velocity to redis.");

    //
    // Send Telemetry to RabbitMQ
    //
    if let Ok(msg) = serde_json::to_vec(&position_item) {
        let _ = mq_channel
            .basic_publish(
                crate::amqp::EXCHANGE_NAME_TELEMETRY,
                crate::amqp::ROUTING_KEY_NETRID_POSITION,
                lapin::options::BasicPublishOptions::default(),
                &msg,
                lapin::BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                rest_warn!("(process_basic_message) could not push aircraft id to RabbitMQ: {e}.");
            });

        rest_debug!("(process_basic_message) pushed aircraft position to RabbitMQ.");
    } else {
        rest_warn!("(process_basic_message) could not serialize position item.");
    }

    //
    // Send Telemetry to RabbitMQ
    //
    if let Ok(msg) = serde_json::to_vec(&velocity_item) {
        let _ = mq_channel
            .basic_publish(
                crate::amqp::EXCHANGE_NAME_TELEMETRY,
                crate::amqp::ROUTING_KEY_NETRID_VELOCITY,
                lapin::options::BasicPublishOptions::default(),
                &msg,
                lapin::BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                rest_warn!("(process_basic_message) could not push aircraft id to RabbitMQ: {e}.");
            });

        rest_debug!("(process_basic_message) pushed aircraft position to RabbitMQ.");
    } else {
        rest_warn!("(process_basic_message) could not serialize velocity item.");
    }

    Ok(())
}

/// Remote ID
#[utoipa::path(
    post,
    path = "/telemetry/netrid",
    tag = "svc-telemetry",
    request_body = Vec<u8>,
    responses(
        (status = 200, description = "Telemetry received."),
        (status = 400, description = "Malformed packet."),
        (status = 500, description = "Something went wrong."),
        (status = 503, description = "Dependencies of svc-telemetry were down."),
    )
)]
pub async fn network_remote_id(
    Extension(mut tlm_pools): Extension<TelemetryPools>,
    Extension(gis_pool): Extension<GisPool>,
    Extension(mq_channel): Extension<lapin::Channel>,
    Extension(claim): Extension<crate::rest::api::jwt::Claim>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(network_remote_id) entry.");

    let payload = <[u8; REMOTE_ID_PACKET_LENGTH]>::try_from(payload.as_ref()).map_err(|_| {
        rest_warn!("(network_remote_id) could not parse payload.");
        StatusCode::BAD_REQUEST
    })?;

    let key = crate::cache::bytes_to_key(&payload);
    let count = tlm_pools
        .netrid
        .increment(&key, CACHE_EXPIRE_MS_NETRID)
        .await
        .map_err(|_| {
            rest_warn!("(network_remote_id) could not increment key.");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match count.cmp(&N_REPORTERS_NEEDED) {
        Ordering::Less => {
            rest_error!("(network_remote_id) netrid reporter count should be impossible: {count}.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ordering::Greater => {
            rest_info!(
                "(network_remote_id) netrid reporter count is greater than needed: {count}."
            );

            // TODO(R4) push up to N reporter confirmations to svc-storage with user_ids
            return Ok(Json(count));
        }
        _ => (), // continue
    }

    let Ok(frame) = Frame::unpack(&payload) else {
        rest_warn!("(network_remote_id) could not parse payload.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let identifier = claim.sub;

    match frame.header.message_type {
        MessageType::Basic => {
            let Ok(msg) = BasicMessage::unpack(&frame.message) else {
                rest_warn!("(network_remote_id) could not parse basic message.");
                return Err(StatusCode::BAD_REQUEST);
            };

            process_basic_message(identifier.clone(), msg, gis_pool, mq_channel).await?;
        }
        crate::msg::netrid::MessageType::Location => {
            let Ok(msg) = LocationMessage::unpack(&frame.message) else {
                rest_warn!("(network_remote_id) could not parse location message.");
                return Err(StatusCode::BAD_REQUEST);
            };

            process_location_message(identifier.clone(), msg, gis_pool, mq_channel).await?;
        }
        _ => {
            rest_warn!(
                "(network_remote_id) unsupported message type: {:#?}.",
                frame.header.message_type
            );
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    Ok(Json(count))
}
