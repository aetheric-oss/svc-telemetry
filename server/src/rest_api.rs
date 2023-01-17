//! REST API implementations for svc-telemetry

use crate::grpc_clients::GrpcClients;
use axum::{extract::Extension, Json};

/// Types Used in REST Messages
pub mod rest_types {
    include!("../../openapi/types.rs");
}
pub use rest_types::AdsbPacket;

// /// Writes an info! message to the app::req logger
// macro_rules! req_info {
//     ($($arg:tt)+) => {
//         log::info!(target: "app::req", $($arg)+);
//     };
// }

// /// Writes an error! message to the app::req logger
// macro_rules! req_error {
//     ($($arg:tt)+) => {
//         log::error!(target: "app::req", $($arg)+);
//     };
// }

/// Writes a debug! message to the app::req logger
macro_rules! req_debug {
    ($($arg:tt)+) => {
        log::debug!(target: "app::req", $($arg)+);
    };
}

/// Post ADS-B Telemetry
#[utoipa::path(
    post,
    path = "/tlm/adsb",
    request_body = AdsbPacket,
    responses(
        (status = 200, description = "Telemetry received."),
    )
)]
pub async fn adsb(
    Extension(mut _grpc_clients): Extension<GrpcClients>,
    Json(_payload): Json<AdsbPacket>,
) -> Result<Json<bool>, (hyper::StatusCode, String)> {
    req_debug!("(adsb) entry");

    // TODO Push to svc-storage
    // TODO Push to third party node

    Ok(Json(true))
}
