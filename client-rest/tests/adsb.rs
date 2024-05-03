// use futures_lite::stream::StreamExt;
use hyper::client::connect::HttpConnector;
use hyper::StatusCode;
use hyper::{Body, Client, Method, Request};
use lib_common::grpc::get_endpoint_from_env;
use packed_struct::prelude::*;
use svc_telemetry_client_rest::adsb_types::*;

#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(endian = "msb", bit_numbering = "msb0", size_bits = "51")]
pub struct PositionData {
    #[packed_field(bits = "0..=1", size_bits = "2")]
    pub surveillance_status: u8,

    #[packed_field(size_bits = "1")]
    pub nic_supplement: u8,

    #[packed_field(size_bits = "12")]
    pub altitude: u16,

    #[packed_field(size_bits = "1")]
    pub time_flag: u8,

    #[packed_field(size_bits = "1")]
    pub cpr_flag: u8,

    #[packed_field(size_bits = "17")]
    pub cpr_latitude: u32,

    #[packed_field(size_bits = "17")]
    pub cpr_longitude: u32,
}

#[derive(PackedStruct, Debug, Clone, Copy, PartialEq)]
#[packed_struct(endian = "msb", bit_numbering = "msb0", size_bytes = "14")]
pub struct PositionFrame {
    #[packed_field(size_bits = "5")]
    pub downlink_format: u8,

    #[packed_field(size_bits = "3")]
    pub capability: u8,

    #[packed_field(size_bits = "24")]
    pub icao_address: u32,

    #[packed_field(size_bits = "5")]
    pub type_code: u8,

    #[packed_field(size_bits = "51")]
    pub data: PositionData,

    #[packed_field(size_bits = "24")]
    pub crc: u32,
}

impl PositionFrame {
    pub fn new(
        icao_address: u32,
        cpr_flag: u8,
        altitude_m: f32,
        latitude: f64,
        longitude: f64,
    ) -> Result<[u8; 14], ()> {
        let altitude = encode_altitude(altitude_m);
        let (cpr_latitude, cpr_longitude) =
            encode_cpr(cpr_flag, longitude, latitude).map_err(|_| ())?;

        let data = PositionData {
            surveillance_status: 0,
            nic_supplement: 0,
            altitude,
            time_flag: 0,
            cpr_flag,
            cpr_latitude,
            cpr_longitude,
        };

        // CRC is not calculated for this test
        let crc = 0;

        let packet = Self {
            downlink_format: 17,
            capability: 5,
            icao_address,
            type_code: 9,
            data,
            crc,
        }
        .pack()
        .map_err(|_| ())?;

        Ok(packet)
    }
}

async fn test_adsb_position(client: &Client<HttpConnector>, url: &str) -> Result<(), ()> {
    let mut odd_flag: u8 = 1;

    let icao_address = 0x123456;
    let altitude_m = 1000.0;
    let latitude = 37.0;
    let longitude = -122.0;

    let payload_1 = PositionFrame::new(icao_address, odd_flag, altitude_m, latitude, longitude)
        .map_err(|_| ())?;

    odd_flag ^= 1;

    let payload_2 = PositionFrame::new(
        icao_address,
        odd_flag,
        altitude_m + 0.1,
        latitude + 0.1,
        longitude + 0.1,
    )
    .map_err(|_| ())?;

    let packets = vec![payload_1, payload_2];
    for payload in packets {
        let request: Request<Body> = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header("content-type", "application/octet-stream")
            .body(hyper::body::Bytes::from(payload.to_vec()).into())
            .unwrap();

        let response = client.request(request).await.map_err(|_| ())?;
        if response.status() == StatusCode::OK {
            println!("OK");
        } else {
            println!("ERROR: {:?}", response.status());
        }
    }

    Ok(())
}

pub async fn test_adsb() {
    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .build_http();

    let (_, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_REST");
    let url = format!("http://web-server:{port}/telemetry/adsb");
    let _ = test_adsb_position(&client, &url).await;
}
