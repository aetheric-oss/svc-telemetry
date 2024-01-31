//! Simulates a flow of ADS-B with multiple reporters

use futures_lite::stream::StreamExt;
use hyper::StatusCode;
use hyper::{Body, Client, Method, Request};
use lib_common::grpc::get_endpoint_from_env;

async fn mq_listener() -> Result<(), ()> {
    let mq_addr = format!("amqp://rabbitmq:5672");

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

    let uri = format!("{}/telemetry/adsb", url);

    // TODO(R4): different reporter ID

    let mut count: u8 = 0;
    let mut odd_flag = 1;
    loop {
        // Using example from 3.1: https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf
        let payload = match odd_flag {
            0 => [
                0x8D, 0x40, 0x62, 0x1D, 0x58, 0xC3, 0x82, 0xD6, 0x90, 0xC8, 0xAC, 0x28, 0x63, count,
            ],
            _ => [
                0x8D, 0x40, 0x62, 0x1D, 0x58, 0xC3, 0x86, 0x43, 0x5C, 0xC4, 0x12, 0x69, 0x2A, count,
            ],
        };

        count += 1;
        odd_flag = 1 - odd_flag;

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

    std::thread::sleep(std::time::Duration::from_secs(5));

    let reporters = 3;
    for _ in 0..reporters {
        tokio::spawn(adsb(url.clone()));
        std::thread::sleep(std::time::Duration::from_millis(225)); // slight lag
    }

    std::thread::sleep(std::time::Duration::from_secs(10));

    Ok(())
}
