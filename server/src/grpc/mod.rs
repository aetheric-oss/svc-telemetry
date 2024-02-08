//! gRPC
//! provides client and server implementations for gRPC

use crate::grpc::client::GrpcClients;
use log::warn;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use svc_gis_client_grpc::client::{
    AircraftId, AircraftPosition, AircraftVelocity, UpdateAircraftIdRequest,
    UpdateAircraftPositionRequest, UpdateAircraftVelocityRequest,
};
use svc_gis_client_grpc::prelude::*;
use tonic::async_trait;

#[macro_use]
pub mod macros;
pub mod client;
pub mod server;

/// gRPC batch loop, empty a ring buffer and push to gRPC at a
///  given cadence and max message size.
#[derive(Debug, Clone)]
pub struct Batch<K> {
    /// Name of the batch
    pub name: String,

    /// gRPC clients
    pub grpc_clients: GrpcClients,

    /// Ring buffer to read from
    pub ring: Arc<Mutex<VecDeque<K>>>,

    /// Cadence in milliseconds
    pub cadence_ms: Duration,

    /// Maximum message size in bytes
    pub max_message_size_bytes: u16,
}

/// Contains the getter functions necessary for a batch loop
#[async_trait]
pub trait IsBatch<T> {
    /// Get the name of the batch
    fn get_name(&self) -> String;

    /// Get the maximum message size in bytes
    fn get_max_message_size_bytes(&self) -> usize;

    /// Get the cadence in milliseconds
    fn get_cadence_ms(&self) -> Duration;

    /// Get the ring buffer
    fn get_ring(&self) -> Arc<Mutex<VecDeque<T>>>;

    /// Get the maximum number of items
    fn get_max_items(&self) -> usize;
}

impl<T> IsBatch<T> for Batch<T> {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_max_message_size_bytes(&self) -> usize {
        self.max_message_size_bytes as usize
    }

    fn get_cadence_ms(&self) -> Duration {
        self.cadence_ms
    }

    fn get_ring(&self) -> Arc<Mutex<VecDeque<T>>> {
        self.ring.clone()
    }

    fn get_max_items(&self) -> usize {
        self.get_max_message_size_bytes() / std::mem::size_of::<T>()
    }
}

/// gRPC batch loop trait, can be started with periodic data pushes
#[async_trait]
pub trait BatchLoop<T>: IsBatch<T> {
    /// Push the ring buffer to gRPC
    async fn push(&mut self) -> Result<(), ()>;

    /// Start the batch loop
    async fn start(&mut self) {
        let name = self.get_name();
        grpc_info!("(gis_batch_loop_{name}) gis_batch_loop entry.");

        let cadence_ms = self.get_cadence_ms(); //Duration::from_millis(cadence_ms as u64);
        let mut start = SystemTime::now();

        loop {
            let Ok(elapsed) = start.elapsed() else {
                grpc_warn!("(gis_batch_loop) Could not get elapsed time.");
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

            let _ = self.push().await;

            // let Ok(_elapsed) = start.elapsed() else {
            //     warn!("(gis_batch_loop) Could not get elapsed time.");
            //     continue;
            // };

            // debug!(
            //     "(gis_batch_loop) push to svc-gis took {:?}.",
            //     elapsed
            // );
        }
    }
}

#[async_trait]
impl BatchLoop<AircraftPosition> for Batch<AircraftPosition> {
    async fn push(&mut self) -> Result<(), ()> {
        let mut data = UpdateAircraftPositionRequest::default(); // UpdateAircraftPositionRequest
        if let Ok(mut ring) = self.get_ring().try_lock() {
            let n_elements = std::cmp::min(self.get_max_items(), ring.len());
            let aircraft: Vec<AircraftPosition> = ring.drain(0..n_elements).collect();
            data.aircraft = aircraft;
        }

        if data.aircraft.is_empty() {
            return Ok(());
        }

        match self
            .grpc_clients
            .gis
            .update_aircraft_position(data.clone())
            .await
        {
            Ok(_) => {
                grpc_info!(
                    "(gis_batch_loop) push to svc-gis succeeded: {} items.",
                    data.aircraft.len()
                );
                Ok(())
            }
            Err(e) => {
                grpc_warn!("(gis_batch_loop) push to svc-gis failed: {}.", e);
                self.grpc_clients.gis.invalidate().await;
                Err(())
            }
        }
    }
}

#[async_trait]
impl BatchLoop<AircraftId> for Batch<AircraftId> {
    async fn push(&mut self) -> Result<(), ()> {
        let mut data = UpdateAircraftIdRequest::default(); // UpdateAircraftPositionRequest
        if let Ok(mut ring) = self.get_ring().try_lock() {
            let n_elements = std::cmp::min(self.get_max_items(), ring.len());
            let aircraft = ring.drain(0..n_elements).collect();
            data.aircraft = aircraft;
        }

        if data.aircraft.is_empty() {
            return Ok(());
        }

        match self.grpc_clients.gis.update_aircraft_id(data.clone()).await {
            Ok(_) => {
                grpc_info!(
                    "(gis_batch_loop) push to svc-gis succeeded: {} items.",
                    data.aircraft.len()
                );
                Ok(())
            }
            Err(e) => {
                grpc_warn!("(gis_batch_loop) push to svc-gis failed: {}.", e);
                self.grpc_clients.gis.invalidate().await;
                Err(())
            }
        }
    }
}

#[async_trait]
impl BatchLoop<AircraftVelocity> for Batch<AircraftVelocity> {
    async fn push(&mut self) -> Result<(), ()> {
        let mut data = UpdateAircraftVelocityRequest::default(); // UpdateAircraftPositionRequest
        if let Ok(mut ring) = self.get_ring().try_lock() {
            let n_elements = std::cmp::min(self.get_max_items(), ring.len());
            let aircraft = ring.drain(0..n_elements).collect();
            data.aircraft = aircraft;
        }

        if data.aircraft.is_empty() {
            return Ok(());
        }

        match self
            .grpc_clients
            .gis
            .update_aircraft_velocity(data.clone())
            .await
        {
            Ok(_) => {
                grpc_info!(
                    "(gis_batch_loop) push to svc-gis succeeded: {} items.",
                    data.aircraft.len()
                );
                Ok(())
            }
            Err(e) => {
                grpc_warn!("(gis_batch_loop) push to svc-gis failed: {}.", e);
                self.grpc_clients.gis.invalidate().await;
                Err(())
            }
        }
    }
}

/// Starts all of the gRPC batch loops for this microservice
pub fn start_batch_loops(
    id_ring: Arc<Mutex<VecDeque<AircraftId>>>,
    position_ring: Arc<Mutex<VecDeque<AircraftPosition>>>,
    velocity_ring: Arc<Mutex<VecDeque<AircraftVelocity>>>,
    config: &crate::Config,
) {
    let grpc_clients_base = GrpcClients::default(config.clone());

    let ring = Arc::clone(&id_ring);
    let max_message_size_bytes = config.gis_max_message_size_bytes;
    let cadence_ms = Duration::from_millis(config.gis_push_cadence_ms as u64);
    let grpc_clients = grpc_clients_base.clone();
    tokio::spawn(async move {
        Batch::<AircraftId> {
            name: "aircraft_id".to_string(),
            grpc_clients,
            ring,
            cadence_ms,
            max_message_size_bytes,
        }
        .start()
        .await
    });

    let ring = Arc::clone(&position_ring);
    let max_message_size_bytes = config.gis_max_message_size_bytes;
    let cadence_ms = Duration::from_millis(config.gis_push_cadence_ms as u64);
    let grpc_clients = grpc_clients_base.clone();
    tokio::spawn(async move {
        Batch::<AircraftPosition> {
            name: "aircraft_position".to_string(),
            grpc_clients,
            ring,
            cadence_ms,
            max_message_size_bytes,
        }
        .start()
        .await
    });

    let ring = Arc::clone(&velocity_ring);
    let max_message_size_bytes = config.gis_max_message_size_bytes;
    let cadence_ms = Duration::from_millis(config.gis_push_cadence_ms as u64);
    let grpc_clients = grpc_clients_base.clone();
    tokio::spawn(async move {
        Batch::<AircraftVelocity> {
            name: "aircraft_velocity".to_string(),
            grpc_clients,
            ring,
            cadence_ms,
            max_message_size_bytes,
        }
        .start()
        .await
    });
}
