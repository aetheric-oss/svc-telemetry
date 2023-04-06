//! Example communication with this service
use hyper::{client::connect::HttpConnector, Body, Client, Method, Request, Response};
use hyper::{Error, StatusCode};
use svc_telemetry_client_rest::types::{MavFrame, MavMessage, MavlinkVersion, ADSB_VEHICLE_DATA};

async fn evaluate(
    response: Result<Response<Body>, Error>,
    expected_code: StatusCode,
    expected_count: i64,
) {
    let Ok(response) = response else {
        println!("Response was an Err() type: {:?}", response.unwrap_err());
        return;
    };

    let status = response.status();

    if status != expected_code {
        println!("expected code: {}, actual: {}", expected_code, status);
        return;
    }

    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let reported_count: i64 = serde_json::from_slice(&bytes).unwrap();

    if reported_count != expected_count {
        println!(
            "expected count: {}, actual: {}",
            expected_count, reported_count
        );
        return;
    }

    println!("{} (body: {})", status.to_string(), reported_count);
}

async fn mavlink(url: &str, client: &Client<HttpConnector>) {
    let uri = format!("{}/telemetry/mavlink/adsb", url);
    let max: u8 = 4;

    // POST /telemetry/mavlink/adsb NOMINAL
    println!(
        "Send {} packets with different headers, expect \
        response body value of 1 each time",
        max + 1
    );
    {
        for count in 0..=max {
            let header = mavlink::MavHeader {
                system_id: 10,
                component_id: 0,
                sequence: count,
            };

            let frame = MavFrame::<MavMessage> {
                header,
                msg: MavMessage::ADSB_VEHICLE(ADSB_VEHICLE_DATA::default()),
                protocol_version: MavlinkVersion::V2,
            };

            let bytes = frame.ser();
            let req = Request::builder()
                .method(Method::POST)
                .uri(uri.clone())
                .header("content-type", "application/octet-stream")
                .body(Body::from(bytes.clone()))
                .unwrap();

            let resp = client.request(req).await;

            // Expect this packet to be the first of its kind in the redis cache
            //  (return value of 1)
            evaluate(resp, StatusCode::OK, 1).await;
        }
    }

    // POST /telemetry/mavlink/adsb REPEAT MESSAGES
    let frame = MavFrame::<MavMessage> {
        header: mavlink::MavHeader {
            system_id: 10,
            component_id: 0,
            sequence: max,
        },
        msg: MavMessage::ADSB_VEHICLE(ADSB_VEHICLE_DATA::default()),
        protocol_version: MavlinkVersion::V2,
    };

    let bytes = frame.ser();

    println!(
        "Send the most recent packet again a few more times, \
        expect incrementing response body values."
    );
    // Send the last packet (same header) a few more times
    // expect the return values to be 2, 3, 4, etc. for each repeated packet
    for expected_count in 2..=6 {
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/octet-stream")
            .body(Body::from(bytes.clone()))
            .unwrap();

        let resp = client.request(req).await;
        evaluate(resp, StatusCode::OK, expected_count).await;
    }

    println!(
        "Wait until after the expiration time, re-send and confirm \
    this was received for the first time"
    );
    std::thread::sleep(std::time::Duration::from_millis(5000));
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri.clone())
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes))
        .unwrap();

    let resp = client.request(req).await;

    // Expect response of "1", received for the first time
    evaluate(resp, StatusCode::OK, 1).await;
}

async fn adsb(url: &str, client: &Client<HttpConnector>) {
    let uri = format!("{}/telemetry/aircraft/adsb", url);
    let max: u8 = 4;

    // POST /telemetry/mavlink/adsb NOMINAL
    println!(
        "Send {} packets with different headers, expect \
        response body value of 1 each time",
        max + 1
    );
    {
        for count in 0..=max {
            let payload: [u8; 14] = [
                0x8D, 0x48, 0x40, count, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
            ];

            let req = Request::builder()
                .method(Method::POST)
                .uri(uri.clone())
                .header("content-type", "application/octet-stream")
                .body(Body::from(payload.clone().to_vec()))
                .unwrap();

            let resp = client.request(req).await;

            // Expect this packet to be the first of its kind in the redis cache
            //  (return value of 1)
            evaluate(resp, StatusCode::OK, 1).await;
        }
    }

    // POST /telemetry/mavlink/adsb REPEAT MESSAGES
    let payload: [u8; 14] = [
        0x8D, 0x48, 0x40, max, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
    ];

    println!(
        "Send the most recent packet again a few more times, \
        expect incrementing response body values."
    );
    // Send the last packet (same header) a few more times
    // expect the return values to be 2, 3, 4, etc. for each repeated packet
    for expected_count in 2..=6 {
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/octet-stream")
            .body(Body::from(payload.clone().to_vec()))
            .unwrap();

        let resp = client.request(req).await;
        evaluate(resp, StatusCode::OK, expected_count).await;
    }

    println!(
        "Wait until after the expiration time, re-send and confirm \
    this was received for the first time"
    );
    std::thread::sleep(std::time::Duration::from_millis(10_000));
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri.clone())
        .header("content-type", "application/octet-stream")
        .body(Body::from(payload.clone().to_vec()))
        .unwrap();

    let resp = client.request(req).await;

    // Expect response of "1", received for the first time
    evaluate(resp, StatusCode::OK, 1).await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "NOTE: Ensure the server and redis containers are running, or this example will fail."
    );

    let host = std::env::var("SERVER_HOSTNAME").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT_REST").unwrap_or_else(|_| "8007".to_string());

    let url = format!("http://{host}:{port}");
    println!("{url}");
    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .build_http();

    mavlink(&url, &client).await;

    // Requires connection to svc-storage
    // cd arrow-air/tools/local-dev && docker compose up svc-storage
    adsb(&url, &client).await;

    Ok(())
}
