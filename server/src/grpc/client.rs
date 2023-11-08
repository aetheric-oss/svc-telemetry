//! gRPC client helpers implementation
use svc_gis_client_grpc::prelude::GisClient;
use svc_storage_client_grpc::prelude::{Client as StorageClient, Clients};
use tokio::sync::OnceCell;

pub(crate) static CLIENTS: OnceCell<GrpcClients> = OnceCell::const_new();

/// Returns CLIENTS, a GrpcClients object with default values.
/// Uses host and port configurations using a Config object generated from
/// environment variables.
/// Initializes CLIENTS if it hasn't been initialized yet.
pub async fn get_clients() -> &'static GrpcClients {
    CLIENTS
        .get_or_init(|| async move {
            let config = crate::Config::try_from_env().unwrap_or_default();
            GrpcClients::default(config)
        })
        .await
}

/// Struct to hold all gRPC client connections
#[derive(Clone, Debug)]
pub struct GrpcClients {
    /// All clients enabled from the svc_storage_grpc_client module
    pub storage: Clients,
    /// A GrpcClient provided by the svc_gis_grpc_client module
    pub gis: GisClient,
}

impl GrpcClients {
    /// Create new GrpcClients with defaults
    pub fn default(config: crate::config::Config) -> Self {
        let storage_clients = Clients::new(config.storage_host_grpc, config.storage_port_grpc);

        GrpcClients {
            storage: storage_clients,
            gis: GisClient::new_client(&config.gis_host_grpc, config.gis_port_grpc, "gis"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grpc_clients_default() {
        crate::get_log_handle().await;
        ut_info!("(test_grpc_clients_default) Start.");

        let clients = get_clients().await;

        let adsb = &clients.storage.adsb;
        ut_debug!("(test_grpc_clients_default) adsb: {:?}", adsb);
        assert_eq!(adsb.get_name(), "adsb");

        let gis = &clients.gis;
        ut_debug!("(test_grpc_clients_default) gis: {:?}", gis);
        assert_eq!(gis.get_name(), "gis");

        ut_info!("(test_grpc_clients_default) Success.");
    }
}
