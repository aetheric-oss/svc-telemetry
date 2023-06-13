#[tokio::test]
async fn test_grpc_server_start() {
    use svc_telemetry::config::Config;
    use svc_telemetry::grpc::server::*;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        grpc_server(Config::default(), Some(shutdown_rx)).await;
    });

    shutdown_tx.send(()).expect("Could not stop server.");
}
