//! gRPC client helpers implementation
//!
pub use svc_storage_client_grpc::adsb::rpc_service_client::RpcServiceClient as AdsbClient;

use futures::lock::Mutex;
use std::sync::Arc;
pub use tonic::transport::Channel;

#[derive(Clone, Debug)]
pub struct GrpcClients {
    /// Svc-Storage ADS-B Client
    pub adsb: GrpcClient<AdsbClient<Channel>>,
}

#[derive(Debug, Clone)]
pub struct GrpcClient<T> {
    inner: Arc<Mutex<Option<T>>>,
    address: String,
}

impl<T> GrpcClient<T> {
    pub async fn invalidate(&mut self) {
        let arc = Arc::clone(&self.inner);
        let mut client = arc.lock().await;
        *client = None;
    }

    pub fn new(env_host: &str, env_port: u16) -> Self {
        let opt: Option<T> = None;
        GrpcClient {
            inner: Arc::new(Mutex::new(opt)),
            address: format!("http://{env_host}:{env_port}"),
        }
    }
}

macro_rules! grpc_client {
    ( $client: ident, $name: expr ) => {
        impl GrpcClient<$client<Channel>> {
            pub async fn get_client(&mut self) -> Option<$client<Channel>> {
                grpc_info!("(get_client) storage::{} entry.", $name);

                let arc = Arc::clone(&self.inner);

                // if already connected, return the client
                let client = arc.lock().await;
                if client.is_some() {
                    return client.clone();
                }

                grpc_info!(
                    "(grpc) connecting to {} server at {}.",
                    $name,
                    self.address.clone()
                );
                let result = $client::connect(self.address.clone()).await;
                match result {
                    Ok(client) => {
                        grpc_info!(
                            "(grpc) success: connected to {} server at {}.",
                            $name,
                            self.address.clone()
                        );
                        Some(client)
                    }
                    Err(e) => {
                        grpc_error!(
                            "(grpc) couldn't connect to {} server at {}; {}.",
                            $name,
                            self.address,
                            e
                        );
                        None
                    }
                }
            }
        }
    };
}

grpc_client!(AdsbClient, "adsb");

impl GrpcClients {
    pub fn default(config: crate::config::Config) -> Self {
        GrpcClients {
            adsb: GrpcClient::<AdsbClient<Channel>>::new(
                &config.storage_host_grpc,
                config.storage_port_grpc,
            ),
        }
    }
}
