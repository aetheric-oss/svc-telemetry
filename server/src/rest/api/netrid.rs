//! Remote ID REST API (Network Remote ID)
//!  Remote ID is a system for identifying and locating drones.
//!  It will be required for use of U-Space airspace by unmanned aircraft.
//! Endpoints for updating aircraft positions

use crate::cache::RedisPools;
// use crate::grpc::client::GrpcClients;
use crate::msg::netrid::{
    BasicMessage, Frame, LocationMessage, MessageType, UaType as NetridAircraftType,
};
use svc_gis_client_grpc::client::{
    AircraftId, AircraftPosition, AircraftType as GisAircraftType, AircraftVelocity, PointZ,
};

use axum::{body::Bytes, extract::Extension, Json};
use chrono::Utc;
use hyper::StatusCode;
use lib_common::time::Timestamp;
use packed_struct::PackedStruct;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Remote ID entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_NETRID: u32 = 10000;

/// Number of times a packet must be received
///  from unique senders before it is considered valid
const N_REPORTERS_NEEDED: u32 = 1;

/// Length of a remote id packet
const REMOTE_ID_PACKET_LENGTH: usize = 25;

impl From<NetridAircraftType> for GisAircraftType {
    fn from(t: NetridAircraftType) -> Self {
        match t {
            NetridAircraftType::Undeclared => GisAircraftType::Undeclared,
            NetridAircraftType::Aeroplane => GisAircraftType::Aeroplane,
            NetridAircraftType::Rotorcraft => GisAircraftType::Rotorcraft,
            NetridAircraftType::Gyroplane => GisAircraftType::Gyroplane,
            NetridAircraftType::HybridLift => GisAircraftType::Hybridlift,
            NetridAircraftType::Ornithopter => GisAircraftType::Ornithopter,
            NetridAircraftType::Glider => GisAircraftType::Glider,
            NetridAircraftType::Kite => GisAircraftType::Kite,
            NetridAircraftType::FreeBalloon => GisAircraftType::Freeballoon,
            NetridAircraftType::CaptiveBalloon => GisAircraftType::Captiveballoon,
            NetridAircraftType::Airship => GisAircraftType::Airship,
            NetridAircraftType::Unpowered => GisAircraftType::Unpowered,
            NetridAircraftType::Rocket => GisAircraftType::Rocket,
            NetridAircraftType::Tethered => GisAircraftType::Tethered,
            NetridAircraftType::GroundObstacle => GisAircraftType::Groundobstacle,
            NetridAircraftType::Other => GisAircraftType::Other,
        }
    }
}

/// Processes a basic remote id message type
async fn process_basic_message(
    _identifier: String,
    message: BasicMessage,
    id_ring: Arc<Mutex<VecDeque<AircraftId>>>,
) -> Result<(), StatusCode> {
    let aircraft_type = GisAircraftType::from(message.ua_type) as i32;

    // TODO(R5): Compare the identifier given for the JWT with the identifier in the message
    let Ok(identifier) = String::from_utf8(message.uas_id.to_vec()) else {
        rest_warn!("(process_basic_message) could not parse identifier to string.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let id_item = AircraftId {
        identifier,
        aircraft_type,
        timestamp_network: Some(Utc::now().into()),
    };

    match id_ring.try_lock() {
        Ok(mut ring) => {
            rest_debug!("(process_basic_message) pushing to id ring buffer.");
            ring.push_back(id_item);
            Ok(())
        }
        _ => {
            rest_warn!("(process_basic_message) could not push to ring buffer.");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Processes a basic remote id message type
async fn process_location_message(
    identifier: String,
    message: LocationMessage,
    position_ring: Arc<Mutex<VecDeque<AircraftPosition>>>,
    velocity_ring: Arc<Mutex<VecDeque<AircraftVelocity>>>,
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

    let timestamp_aircraft: Option<Timestamp> = match message.decode_timestamp() {
        Ok(ts) => Some(ts.into()),
        Err(_) => None,
    };

    let latitude = message.decode_latitude();
    let longitude = message.decode_longitude();

    let position_item = AircraftPosition {
        identifier: identifier.clone(),
        geom: Some(PointZ {
            latitude,
            longitude,
            altitude_meters,
        }),
        timestamp_network: Some(Utc::now().into()),
        timestamp_aircraft: None,
    };

    let velocity_item = AircraftVelocity {
        identifier,
        velocity_vertical_mps,
        velocity_horizontal_ground_mps,
        velocity_horizontal_air_mps: None,
        track_angle_degrees: message.decode_direction() as f32,
        timestamp_aircraft,
        timestamp_network: Some(Utc::now().into()),
    };

    match position_ring.try_lock() {
        Ok(mut ring) => {
            rest_debug!("(process_basic_message) pushing to position ring buffer.");
            ring.push_back(position_item);
        }
        _ => {
            rest_warn!("(process_basic_message) could not push to ring buffer.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    match velocity_ring.try_lock() {
        Ok(mut ring) => {
            rest_debug!("(process_basic_message) pushing to velocity ring buffer.");
            ring.push_back(velocity_item);
            Ok(())
        }
        _ => {
            rest_warn!("(process_basic_message) could not push to ring buffer.");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
    Extension(mut pools): Extension<RedisPools>,
    // Extension(_mq_channel): Extension<lapin::Channel>,
    // Extension(_grpc_clients): Extension<GrpcClients>,
    Extension(position_ring): Extension<Arc<Mutex<VecDeque<AircraftPosition>>>>,
    Extension(id_ring): Extension<Arc<Mutex<VecDeque<AircraftId>>>>,
    Extension(velocity_ring): Extension<Arc<Mutex<VecDeque<AircraftVelocity>>>>,
    Extension(identifier): Extension<String>,
    payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    rest_info!("(network_remote_id) entry.");

    let Ok(payload) = <[u8; REMOTE_ID_PACKET_LENGTH]>::try_from(payload.as_ref()) else {
        rest_warn!("(network_remote_id) could not parse payload.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let Ok(key) = std::str::from_utf8(&payload[..]) else {
        rest_warn!("(network_remote_id) could not parse payload.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let result = pools.netrid.increment(key, CACHE_EXPIRE_MS_NETRID).await;

    let Ok(count) = result else {
        rest_warn!("(network_remote_id) could not increment key.");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

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

    match frame.header.message_type {
        MessageType::Basic => {
            let Ok(msg) = BasicMessage::unpack(&frame.message) else {
                rest_warn!("(network_remote_id) could not parse basic message.");
                return Err(StatusCode::BAD_REQUEST);
            };

            process_basic_message(identifier.clone(), msg, id_ring).await?;
        }
        crate::msg::netrid::MessageType::Location => {
            let Ok(msg) = LocationMessage::unpack(&frame.message) else {
                rest_warn!("(network_remote_id) could not parse location message.");
                return Err(StatusCode::BAD_REQUEST);
            };

            process_location_message(identifier.clone(), msg, position_ring, velocity_ring).await?;
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
