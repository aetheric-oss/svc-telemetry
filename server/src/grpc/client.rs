//! gRPC client helpers implementation

use lib_common::grpc::{Client, GrpcClient};
use svc_gis_client_grpc::client::rpc_service_client::RpcServiceClient as GisClient;
use svc_storage_client_grpc::Clients;
pub use tonic::transport::Channel;

/// Struct to hold all gRPC client connections
#[derive(Clone, Debug)]
pub struct GrpcClients {
    /// svc-storage ADS-B Client
    pub storage: Clients,

    /// svc-gis client
    pub gis: GrpcClient<GisClient<Channel>>,
}

impl GrpcClients {
    /// Create new GrpcClients with defaults
    pub fn default(config: crate::config::Config) -> Self {
        let storage_clients = Clients::new(config.storage_host_grpc, config.storage_port_grpc);

        GrpcClients {
            storage: storage_clients,
            gis: GrpcClient::<GisClient<Channel>>::new_client(
                &config.gis_host_grpc,
                config.gis_port_grpc,
                "gis",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use svc_storage_client_grpc::prelude::*;

    use super::*;

    #[tokio::test]
    async fn test_grpc_clients_default() {
        let config = crate::config::Config::default();
        let clients = GrpcClients::default(config);
        let adsb = &clients.storage.adsb;
        println!("{:?}", adsb);
        assert_eq!(adsb.get_name(), "adsb");

        let gis = &clients.gis;
        println!("{:?}", gis);
        assert_eq!(gis.get_name(), "gis");
    }
}
