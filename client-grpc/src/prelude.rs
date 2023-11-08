//! Re-export of used objects

pub use super::client as telemetry;
pub use super::service::Client as TelemetryServiceClient;
pub use telemetry::TelemetryClient;

pub use lib_common::grpc::Client;
