//! Simulates a flow of ADS-B with multiple reporters

use futures_lite::stream::StreamExt;
use hyper::StatusCode;
use hyper::{Body, Client, Method, Request};
use lib_common::grpc::get_endpoint_from_env;

async fn mq_listener() -> Result<(), ()> {
    let mq_addr = format!("amqp://localhost:5672");

    // Establish connection to RabbitMQ node
    println!("(mq_listener) connecting to MQ server at {}...", mq_addr);
    let result = lapin::Connection::connect(&mq_addr, lapin::ConnectionProperties::default()).await;
    let mq_connection = match result {
        Ok(conn) => conn,
        Err(e) => {
            println!("(mq_listener) could not connect to MQ server at {mq_addr}.");
            println!("(mq_listener) error: {:?}", e);
            return Err(());
        }
    };

    // Create channel
    println!("(mq_listener) creating channel at {}...", mq_addr);
    let mq_channel = match mq_connection.create_channel().await {
        Ok(channel) => channel,
        Err(e) => {
            println!("(mq_listener) could not create channel at {mq_addr}.");
            println!("(mq_listener) error: {:?}", e);
            return Err(());
        }
    };

    let mut consumer = mq_channel
        .basic_consume(
            "adsb",
            "mq_listener",
            lapin::options::BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await
        .unwrap();

    while let Some(delivery) = consumer.next().await {
        println!("received message {:?}", delivery);
    }

    Ok(())
}

async fn adsb(url: String) {
    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .build_http();

    let uri = format!("{}/telemetry/aircraft/adsb", url);

    // TODO(R4): different reporter ID

    let mut count: u8 = 0;
    loop {
        let payload: [u8; 14] = [
            0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, count,
        ];

        count += 1;

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/octet-stream")
            .body(Body::from(payload.clone().to_vec()))
            .unwrap();

        match client.request(req).await {
            Ok(resp) => {
                if resp.status() == StatusCode::OK {
                    println!("OK");
                } else {
                    println!("ERROR: {:?}", resp.status());
                }
            }
            Err(e) => {
                println!("ERROR: {:?}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(500)); // twice a second
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NOTE: Ensure the server is running, or this example will fail.");

    let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_REST");
    let url = format!("http://{host}:{port}");

    println!("Rest endpoint set to [{url}].");

    tokio::spawn(mq_listener());

    let reporters = 3;
    for _ in 0..reporters {
        tokio::spawn(adsb(url.clone()));
        std::thread::sleep(std::time::Duration::from_millis(225)); // slight lag
    }

    std::thread::sleep(std::time::Duration::from_secs(10));

    Ok(())
}
