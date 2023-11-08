//! Gis batch loop module
//!
use crate::grpc::client::GrpcClients;
use log::{info, warn};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use svc_gis_client_grpc::prelude::*;

///
/// Push telemetry to svc-gis in bulk
///
pub async fn gis_batch_loop(
    mut grpc_clients: GrpcClients,
    ring: Arc<Mutex<VecDeque<gis::AircraftPosition>>>,
    cadence_ms: u16,
    max_message_size_bytes: u16,
) {
    info!("(gis_batch_loop) gis_batch_loop entry.");

    let cadence_ms = Duration::from_millis(cadence_ms as u64);
    let mut data = gis::UpdateAircraftPositionRequest::default();
    let mut start = SystemTime::now();
    let max_items = max_message_size_bytes as usize / std::mem::size_of::<gis::AircraftPosition>();

    loop {
        let Ok(elapsed) = start.elapsed() else {
            warn!("(gis_batch_loop) Could not get elapsed time.");
            sleep(cadence_ms);
            continue;
        };

        if elapsed > cadence_ms {
            warn!(
                "(gis_batch_loop) elapsed time ({:?} ms) exceeds cadence ({:?} ms)",
                elapsed, cadence_ms
            );
        } else {
            sleep(cadence_ms - elapsed)
        }

        start = SystemTime::now();

        if let Ok(mut ring) = ring.lock() {
            if !ring.is_empty() {
                let n_elements = std::cmp::min(max_items, ring.len());
                data.aircraft = ring.drain(0..n_elements).collect();
            }
        }

        if data.aircraft.is_empty() {
            continue;
        }

        match grpc_clients
            .gis
            .update_aircraft_position(data.clone())
            .await
        {
            Ok(_) => info!(
                "(gis_batch_loop) push to svc-gis succeeded: {} items.",
                data.aircraft.len()
            ),
            Err(e) => {
                warn!("(gis_batch_loop) push to svc-gis failed: {}.", e);
                grpc_clients.gis.invalidate().await;
                continue;
            }
        }

        let Ok(elapsed) = start.elapsed() else {
            warn!("(gis_batch_loop) Could not get elapsed time.");
            continue;
        };

        info!(
            "(gis_batch_loop) push to svc-gis took {:?} milliseconds.",
            elapsed
        );
    }
}
