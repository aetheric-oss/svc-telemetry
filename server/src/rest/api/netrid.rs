//! Remote ID REST API (Network Remote ID)
//!  Remote ID is a system for identifying and locating drones.
//!  It will be required for use of U-Space airspace by unmanned aircraft.
//! Endpoints for updating aircraft positions

use crate::cache::pool::GisPool;
use crate::cache::TelemetryPools;
use crate::msg::netrid::{
    BasicMessage, Frame, IdType, LocationMessage, MessageType, UaType as NetridAircraftType,
};
use svc_gis_client_grpc::prelude::types::*;

use axum::{body::Bytes, extract::Extension, Json};
use hyper::StatusCode;
use lib_common::time::Utc;
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
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need AMQP and redis backends to test
async fn process_basic_message(
    jwt_identifier: String,
    message: BasicMessage,
    mut gis_pool: GisPool,
    mq_channel: lapin::Channel,
) -> Result<(), StatusCode> {
    rest_debug!("entry.");
    let aircraft_type = AircraftType::from(message.ua_type);
    let mut id_item = AircraftId {
        identifier: Some(jwt_identifier),
        session_id: None,
        aircraft_type,
        timestamp_network: Utc::now(),
        timestamp_asset: None,
    };

    let identifier = String::from_utf8(message.uas_id.to_vec())
        .map_err(|_| {
            rest_warn!("could not parse identifier to string.");
            StatusCode::BAD_REQUEST
        })?
        .trim()
        .to_string();

    match message.id_type {
        IdType::UtmAssigned => id_item.session_id = Some(identifier),
        IdType::SpecificSession => id_item.session_id = Some(identifier),
        _ => id_item.identifier = Some(identifier),
    }

    gis_pool
        .push::<AircraftId>(id_item.clone(), REDIS_KEY_AIRCRAFT_ID)
        .await
        .map_err(|_| {
            rest_warn!("could not push aircraft id to cache.");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    rest_debug!("pushed aircraft id to redis.");

    //
    // Send Telemetry to RabbitMQ
    //
    let msg = match serde_json::to_vec(&id_item) {
        Ok(msg) => msg,
        Err(_) => {
            rest_warn!("could not serialize id item.");
            return Ok(()); // fine, not a critical error
        }
    };

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
            rest_warn!("could not push aircraft id to RabbitMQ: {e}.");
        })
        .map(|_| {
            rest_debug!("pushed aircraft id to RabbitMQ.");
        });

    Ok(())
}

/// Processes a basic remote id message type
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need AMQP and redis backends to test
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

    let altitude_meters = message.decode_altitude().map_err(|e| {
        rest_warn!("could not parse altitude: {e}.");
        StatusCode::BAD_REQUEST
    })?;

    let velocity_horizontal_ground_mps = message.decode_speed().map_err(|e| {
        rest_warn!("could not parse speed: {e}.");
        StatusCode::BAD_REQUEST
    })?;

    let velocity_vertical_mps = message.decode_vertical_speed().map_err(|e| {
        rest_warn!("could not parse vertical speed: {e}.");
        StatusCode::BAD_REQUEST
    })?;

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
            rest_warn!("could not push aircraft position to cache.");
            StatusCode::INTERNAL_SERVER_ERROR
        })?; // TODO(R5): Do we want to bail here or still send the velocity to postgis?

    rest_debug!("pushed aircraft position to redis.");

    let _ = gis_pool
        .push::<AircraftVelocity>(velocity_item.clone(), REDIS_KEY_AIRCRAFT_VELOCITY)
        .await
        .map_err(|_| {
            rest_warn!("could not push aircraft velocity to cache.");
            // StatusCode::INTERNAL_SERVER_ERROR
        });

    rest_debug!("pushed aircraft velocity to redis.");

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
                rest_warn!("could not push aircraft id to RabbitMQ: {e}.");
            });

        rest_debug!("pushed aircraft position to RabbitMQ.");
    } else {
        rest_warn!("could not serialize position item.");
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
                rest_warn!("could not push aircraft id to RabbitMQ: {e}.");
            });

        rest_debug!("pushed aircraft position to RabbitMQ.");
    } else {
        rest_warn!("could not serialize velocity item.");
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
    rest_info!("entry.");

    let payload = <[u8; REMOTE_ID_PACKET_LENGTH]>::try_from(payload.as_ref()).map_err(|_| {
        rest_warn!("could not parse payload.");
        StatusCode::BAD_REQUEST
    })?;

    let frame = Frame::unpack(&payload).map_err(|_| {
        rest_warn!("could not parse payload.");
        StatusCode::BAD_REQUEST
    })?;

    //
    // BasicMessage is identical throughout the whole flight,
    //  don't want to toss repeats of the same message
    let mut count = 1;
    if frame.header.message_type != MessageType::Basic {
        let key = crate::cache::bytes_to_key(&payload);
        count = tlm_pools
            .netrid
            .increment(&key, CACHE_EXPIRE_MS_NETRID)
            .await
            .map_err(|_| {
                rest_warn!("could not increment key.");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        match count.cmp(&N_REPORTERS_NEEDED) {
            Ordering::Less => {
                rest_error!("netrid reporter count should be impossible: {count}.");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ordering::Greater => {
                rest_info!("netrid reporter count is greater than needed: {count}.");
                return Ok(Json(count));
            }
            _ => (), // continue
        }
    }

    // Eventually allow forwarding of packets from other aircraft
    // TODO(R5)
    let jwt_identifier = claim.sub;
    match frame.header.message_type {
        MessageType::Basic => {
            let msg = BasicMessage::unpack(&frame.message).map_err(|_| {
                rest_warn!("could not parse basic message.");
                StatusCode::BAD_REQUEST
            })?;

            process_basic_message(jwt_identifier, msg, gis_pool, mq_channel).await?;
        }
        MessageType::Location => {
            let msg = LocationMessage::unpack(&frame.message).map_err(|_| {
                rest_warn!("could not parse location message.");
                StatusCode::BAD_REQUEST
            })?;

            process_location_message(jwt_identifier, msg, gis_pool, mq_channel).await?;
        }
        _ => {
            rest_warn!(
                "unsupported message type: {:#?}.",
                frame.header.message_type
            );
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    Ok(Json(count))
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::cache::pool::TelemetryPool;
    // use crate::msg::netrid::*;

    #[tokio::test]
    #[cfg(not(feature = "stub_backends"))]
    async fn test_network_remote_id_valid() {
        let mut config = crate::config::Config::default();
        // arbitrary addresses
        config.redis.url = Some("redis://localhost:11111".to_string());
        config.amqp.url = Some("amqp://localhost:5672".to_string());
        let pools = TelemetryPools {
            netrid: TelemetryPool::new(config.clone(), "netrid").await.unwrap(),
            adsb: TelemetryPool::new(config.clone(), "adsb").await.unwrap(),
        };

        let gis_pool = GisPool::new(config.clone()).await.unwrap();
        let mq_channel = crate::amqp::init_mq(config.clone()).await.unwrap();

        let claim = crate::rest::api::jwt::Claim {
            iat: 0,
            sub: "test".to_string(),
            exp: 0,
        };

        // invalid packet length
        let payload = Bytes::from(vec![0; REMOTE_ID_PACKET_LENGTH - 1]);
        let result = network_remote_id(
            Extension(pools.clone()),
            Extension(gis_pool.clone()),
            Extension(mq_channel.clone()),
            Extension(claim.clone()),
            payload,
        )
        .await
        .unwrap_err();
        assert_eq!(result, StatusCode::BAD_REQUEST);

        // invalid/unsupported packet type
        let frame = Frame {
            header: Header {
                message_type: MessageType::MessagePack,
                protocol_version: 0,
            },
            message: BasicMessage {
                ua_type: NetridAircraftType::Aeroplane,
                id_type: IdType::CaaAssigned,
                uas_id: [0; 20],
                ..Default::default()
            }
            .pack()
            .unwrap(),
        };
        let payload = Bytes::from(frame.pack().unwrap().to_vec());
        let result = network_remote_id(
            Extension(pools.clone()),
            Extension(gis_pool.clone()),
            Extension(mq_channel.clone()),
            Extension(claim.clone()),
            payload,
        )
        .await
        .unwrap_err();
        assert_eq!(result, StatusCode::BAD_REQUEST);

        // not matching header type and actual body type
        let frame = Frame {
            header: Header {
                message_type: MessageType::Location,
                protocol_version: 0,
            },
            message: BasicMessage {
                ua_type: NetridAircraftType::Undeclared,
                id_type: IdType::CaaAssigned,
                uas_id: [0; 20],
                ..Default::default()
            }
            .pack()
            .unwrap(),
        };
        let payload = Bytes::from(frame.pack().unwrap().to_vec());
        let result = network_remote_id(
            Extension(pools.clone()),
            Extension(gis_pool.clone()),
            Extension(mq_channel.clone()),
            Extension(claim.clone()),
            payload,
        )
        .await
        .unwrap_err();
        assert_eq!(result, StatusCode::BAD_REQUEST);

        // assert_eq!(result, Ok(Json(1)));
    }

    #[test]
    fn test_aircraft_type() {
        assert_eq!(
            AircraftType::from(NetridAircraftType::Undeclared),
            AircraftType::Undeclared
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Aeroplane),
            AircraftType::Aeroplane
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Rotorcraft),
            AircraftType::Rotorcraft
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Gyroplane),
            AircraftType::Gyroplane
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::HybridLift),
            AircraftType::Hybridlift
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Ornithopter),
            AircraftType::Ornithopter
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Glider),
            AircraftType::Glider
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Kite),
            AircraftType::Kite
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::FreeBalloon),
            AircraftType::Freeballoon
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::CaptiveBalloon),
            AircraftType::Captiveballoon
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Airship),
            AircraftType::Airship
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Unpowered),
            AircraftType::Unpowered
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Rocket),
            AircraftType::Rocket
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Tethered),
            AircraftType::Tethered
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::GroundObstacle),
            AircraftType::Groundobstacle
        );
        assert_eq!(
            AircraftType::from(NetridAircraftType::Other),
            AircraftType::Other
        );
    }
}
