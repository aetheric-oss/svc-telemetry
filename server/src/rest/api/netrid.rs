//! Remote ID REST API (Network Remote ID)
//!  Remote ID is a system for identifying and locating drones.
//!  It will be required for use of U-Space airspace by unmanned aircraft.
//! Endpoints for updating aircraft positions

use crate::cache::pool::RedisPool;
use crate::cache::RedisPools;
use crate::grpc::client::GrpcClients;
use crate::msg::netrid::UaType as NetridAircraftType;
use svc_gis_client_grpc::client::{AircraftPosition, AircraftType as GisAircraftType, Coordinates};
use svc_storage_client_grpc::prelude::*;
use svc_storage_client_grpc::resources::adsb;

use axum::{body::Bytes, extract::Extension, Json};
use chrono::{DateTime, Utc};
use hyper::StatusCode;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Remote ID entries in the cache will expire after 60 seconds
const CACHE_EXPIRE_MS_AIRCRAFT_NETRID: u32 = 10000;

/// Number of times a packet must be received
///  from unique senders before it is considered valid
const N_REPORTERS_NEEDED: u32 = 1;

///
/// Pushes a position telemetry message to the ring buffer
///
pub async fn gis_position_push(
    identifier: String,
    latitude: f64,
    longitude: f64,
    altitude_meters: f32,
    timestamp_aircraft: DateTime<Utc>,
    aircraft_type: NetridAircraftType,
    mut pool: RedisPool,
    ring: Arc<Mutex<VecDeque<AircraftPosition>>>,
) -> Result<(), ()> {
    let aircraft_type: GisAircraftType = match aircraft_type {
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
    };

    let item = AircraftPosition {
        identifier,
        aircraft_type: aircraft_type as i32,
        location: Some(Coordinates {
            latitude,
            longitude,
        }),
        altitude_meters,
        timestamp_aircraft: Some(timestamp_aircraft.into()),
        timestamp_network: Some(Utc::now().into()),
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
    Extension(_pools): Extension<RedisPools>,
    Extension(_mq_channel): Extension<lapin::Channel>,
    Extension(_grpc_clients): Extension<GrpcClients>,
    Extension(_ring): Extension<Arc<Mutex<VecDeque<AircraftPosition>>>>,
    _payload: Bytes,
) -> Result<Json<u32>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}
