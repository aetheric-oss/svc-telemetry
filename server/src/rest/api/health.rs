//! REST API endpoint for health check

use crate::grpc::client::GrpcClients;
use axum::extract::Extension;
use axum::response::IntoResponse;
use hyper::StatusCode;
use svc_gis_client_grpc::prelude::*;
use svc_storage_client_grpc::prelude::*;

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
) -> Result<impl IntoResponse, StatusCode> {
    rest_debug!("(health_check) entry.");

    let mut ok = true;

    if grpc_clients
        .storage
        .adsb
        .is_ready(ReadyRequest {})
        .await
        .is_err()
    {
        let error_msg = "svc-storage adsb unavailable.".to_string();
        rest_error!("(health_check) {}.", &error_msg);
        ok = false;
    }

    if grpc_clients
        .gis
        .is_ready(gis::ReadyRequest {})
        .await
        .is_err()
    {
        let error_msg = "svc-gis unavailable".to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_success() {
        // Mock the GrpcClients extension
        let config = crate::config::Config::default();
        let grpc_clients = GrpcClients::default(config);
        let extension = Extension(grpc_clients);

        // Call the health_check function
        let result = health_check(extension).await;

        // Assert the expected result
        println!("{:?}", result);
        assert!(result.is_ok());
    }
}
