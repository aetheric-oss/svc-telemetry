use log::{info, warn};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use svc_gis_client_grpc::client::AircraftPosition;
use svc_gis_client_grpc::client::UpdateAircraftPositionRequest as PositionRequest;
use svc_gis_client_grpc::{Client, ClientConnect};
use svc_telemetry::grpc::client::GrpcClients;

///
/// Push telemetry to svc-gis in bulk
///
pub async fn gis_batch_loop(
    mut grpc_clients: GrpcClients,
    ring: Arc<Mutex<VecDeque<AircraftPosition>>>,
    cadence_ms: u16,
    max_message_size_bytes: u16,
) {
    info!("(svc-telemetry) gis_batch_loop entry.");

    let cadence_ms = Duration::from_millis(cadence_ms as u64);
    let mut data = PositionRequest::default();
    let mut start = SystemTime::now();
    let max_items = max_message_size_bytes as usize / std::mem::size_of::<AircraftPosition>();

    loop {
        let Ok(elapsed) = start.elapsed() else {
            warn!("(svc-telemetry) Could not get elapsed time.");
            sleep(cadence_ms);
            continue;
        };

        if elapsed > cadence_ms {
            warn!(
                "(svc-telemetry) elapsed time ({:?} ms) exceeds cadence ({:?} ms)",
                elapsed, cadence_ms
            );
        } else {
            sleep(cadence_ms - elapsed)
        }

        start = SystemTime::now();

        // Don't want to drain the ringbuffer if client is unavailable
        // ringbuffer will automatically overwrite oldest entries when it reaches capacity
        let mut client = match grpc_clients.gis.get_client().await {
            Ok(client) => client,
            Err(e) => {
                warn!("(svc-telemetry) svc-gis client not available: {}", e);
                continue;
            }
        };

        if let Ok(mut ring) = ring.lock() {
            if !ring.is_empty() {
                let n_elements = std::cmp::min(max_items, ring.len());
                data.aircraft = ring.drain(0..n_elements).collect();
            }
        }

        if data.aircraft.is_empty() {
            continue;
        }

        let request = tonic::Request::new(data.clone());
        match client.update_aircraft_position(request).await {
            Ok(_) => info!(
                "(svc-telemetry) push to svc-gis succeeded: {} items.",
                data.aircraft.len()
            ),
            Err(e) => {
                warn!("(svc-telemetry) push to svc-gis failed: {}.", e);
                grpc_clients.gis.invalidate().await;
                continue;
            }
        }

        let Ok(elapsed) = start.elapsed() else {
            warn!("(svc-telemetry) Could not get elapsed time.");
            continue;
        };

        info!(
            "(svc-telemetry) push to svc-gis took {:?} milliseconds.",
            elapsed
        );
    }
}
