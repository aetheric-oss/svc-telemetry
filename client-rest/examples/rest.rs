//! Example communication with this service

// use hyper::{Body, Client, Method, Request, Response};
// use hyper::{Error, StatusCode};
// use std::time::{Duration, SystemTime};
// use svc_telemetry_client_rest::types::*;

// fn evaluate(resp: Result<Response<Body>, Error>, expected_code: StatusCode) -> (bool, String) {
//     let mut ok = true;
//     let result_str: String = match resp {
//         Ok(r) => {
//             let tmp = r.status() == expected_code;
//             ok &= tmp;
//             println!("{:?}", r.body());

//             r.status().to_string()
//         }
//         Err(e) => {
//             ok = false;
//             e.to_string()
//         }
//     };

//     (ok, result_str)
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NOTE: Ensure the server is running, or this example will fail.");

    // let rest_port = std::env::var("HOST_PORT_REST").unwrap_or_else(|_| "8000".to_string());

    // // let host_port = env!("HOST_PORT");
    // let url = format!("http://0.0.0.0:{rest_port}");
    // let mut ok = true;
    // let client: Client<HttpConnector> = Client::builder()
    //     .pool_idle_timeout(std::time::Duration::from_secs(10))
    //     .build_http();

    // PUT /telemetry/vertiport/
    // {
    //     let data = VertiportsQuery {
    //         latitude: 32.7262,
    //         longitude: 117.1544,
    //     };
    //     let data_str = serde_json::to_string(&data).unwrap();
    //     let uri = format!("{}/telemetry/vertiport", url);
    //     let req = Request::builder()
    //         .method(Method::PUT)
    //         .uri(uri.clone())
    //         .header("content-type", "application/json")
    //         .body(Body::from(data_str))
    //         .unwrap();

    //     let resp = client.request(req).await;
    //     let (success, result_str) = evaluate(resp, StatusCode::OK);
    //     ok &= success;

    //     println!("{}: {}", uri, result_str);
    // }

    // PUT /telemetry/:aircraft
    // {
    //     let data = TelemetryData {
    //         latitude: 32.7262,
    //         longitude: 117.1544,
    //         altitude_meters: 1000.0,

    //     };
    //     let data_str = serde_json::to_string(&data).unwrap();
    //     let uri = format!("{}/telemetry/vertiport", url);
    //     let req = Request::builder()
    //         .method(Method::PUT)
    //         .uri(uri.clone())
    //         .header("content-type", "application/json")
    //         .body(Body::from(data_str))
    //         .unwrap();

    //     let resp = client.request(req).await;
    //     let (success, result_str) = evaluate(resp, StatusCode::OK);
    //     ok &= success;

    //     println!("{}: {}", uri, result_str);
    // }

    Ok(())
}
