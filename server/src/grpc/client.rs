//! gRPC client helpers implementation
use svc_gis_client_grpc::prelude::Client;
use svc_gis_client_grpc::prelude::GisClient;
use svc_storage_client_grpc::prelude::Clients;

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
    use svc_gis_client_grpc::prelude::Client as GisClient;
    // use svc_storage_client_grpc::prelude::Client as StorageClient;

    #[tokio::test]
    async fn test_grpc_clients_default() {
        lib_common::logger::get_log_handle().await;
        ut_info!("Start.");

        let config = crate::config::Config::default();
        let clients = GrpcClients::default(config);

        let adsb = &clients.storage.adsb;
        ut_debug!("adsb: {:?}", adsb);
        assert_eq!(adsb.get_name(), "adsb");

        let gis = &clients.gis;
        ut_debug!("gis: {:?}", gis);
        assert_eq!(gis.get_name(), "gis");

        ut_info!("Success.");
    }
}
