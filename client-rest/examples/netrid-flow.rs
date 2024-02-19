//! Simulates a flow of ADS-B with multiple reporters

use futures_lite::stream::StreamExt;
use hyper::{body::Bytes, Body, Client, Method, Request, StatusCode};
use lib_common::grpc::get_endpoint_from_env;
use packed_struct::PackedStruct;
use svc_gis_client_grpc::prelude::types::AircraftId;
use svc_telemetry_client_rest::netrid_types::*;

async fn mq_listener() -> Result<(), ()> {
    let mq_addr = format!("amqp://rabbitmq:5672");

    // Establish connection to RabbitMQ node
    println!("(mq_listener) connecting to MQ server at {}...", mq_addr);
    let mq_connection =
        lapin::Connection::connect(&mq_addr, lapin::ConnectionProperties::default())
            .await
            .map_err(|e| {
                println!("(mq_listener) could not connect to MQ server at {mq_addr}.");
                println!("(mq_listener) error: {:?}", e);
            })?;

    // Create channel
    println!("(mq_listener) creating channel at {}...", mq_addr);
    let mq_channel = mq_connection.create_channel().await.map_err(|e| {
        println!("(mq_listener) could not create channel at {mq_addr}.");
        println!("(mq_listener) error: {:?}", e);
    })?;

    let mut consumer = mq_channel
        .basic_consume(
            "netrid_id",
            "mq_listener",
            lapin::options::BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await
        .unwrap();

    while let Some(delivery) = consumer.next().await {
        if let Ok(id) = serde_json::from_slice::<AircraftId>(&delivery.unwrap().data) {
            println!("id: {:?}", id);
        } else {
            println!("error: could not deserialize id message");
        }
    }

    Ok(())
}

async fn netrid(reporter: i32, url: String) -> () {
    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .build_http();

    let uri = format!("{url}/telemetry/netrid");
    let identifier = format!("aircraft{reporter}");

    // FAILED PUSH WITH NO CREDENTIALS
    let payload = Frame {
        header: Header {
            message_type: MessageType::Basic,
            ..Default::default()
        },
        message: BasicMessage {
            id_type: IdType::CaaAssigned,
            ua_type: UaType::Rotorcraft,
            uas_id: <[u8; 20]>::try_from(format!("{:>20}", identifier).as_ref()).unwrap(),
            ..Default::default()
        }
        .pack()
        .unwrap(),
    }
    .pack()
    .unwrap();

    let req = Request::builder()
        .method(Method::POST)
        .uri(uri.clone())
        .header("content-type", "application/octet-stream")
        .body(Body::from(payload.clone().to_vec()))
        .unwrap();

    let _ = match client.request(req).await {
        Ok(resp) => {
            if resp.status() != StatusCode::UNAUTHORIZED {
                panic!(
                    "Got unexpected status code (expected 401 UNAUTHORIZED): {:?}",
                    resp
                );
            }
        }
        Err(e) => {
            println!("Got unexpected error: {e}");
        }
    };

    // LOGIN
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("{url}/telemetry/login"))
        .header("content-type", "text/plain")
        .body(Bytes::from(identifier.clone()).into())
        .unwrap();

    let resp = match client.request(req).await {
        Ok(resp) => {
            if resp.status() != StatusCode::OK {
                panic!("Got unexpected status code (expected 200 OK): {:?}", resp);
            }

            resp
        }
        Err(e) => {
            println!("ERROR: {:?}", e);
            panic!("Could not login.");
        }
    };

    let body = resp.into_body();
    let token = hyper::body::to_bytes(body).await.unwrap();
    let token = String::from_utf8(token.to_vec()).unwrap();
    let token = token.trim_matches('"').replace("\"", "");
    println!("Token: {}", token);

    for _ in 0..10 {
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/octet-stream")
            .header("Authorization", format!("Bearer {token}"))
            .body(Bytes::from(payload.to_vec()).into())
            .unwrap();

        match client.request(req).await {
            Ok(resp) => {
                if resp.status() == StatusCode::OK {
                    println!("{identifier} push: OK");
                } else {
                    panic!("{identifier} push: ERROR {:?}", resp.status());
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

    let reporters = 3;
    for x in 0..reporters {
        tokio::spawn(netrid(x, url.clone()));
        std::thread::sleep(std::time::Duration::from_millis(225)); // slight lag
    }

    let _ = mq_listener().await;

    std::thread::sleep(std::time::Duration::from_secs(10));

    Ok(())
}
