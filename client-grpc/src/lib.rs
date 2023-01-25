//! <center>
//! <img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />
//! </center>
//! <div align="center">
//!     <a href="https://github.com/Arrow-air/svc-telemetry/releases">
//!         <img src="https://img.shields.io/github/v/release/Arrow-air/svc-telemetry?include_prereleases" alt="GitHub release (latest by date including pre-releases)">
//!     </a>
//!     <a href="https://github.com/Arrow-air/svc-telemetry/tree/main">
//!         <img src="https://github.com/arrow-air/svc-telemetry/actions/workflows/rust_ci.yml/badge.svg?branch=main" alt="Rust Checks">
//!     </a>
//!     <a href="https://discord.com/invite/arrow">
//!         <img src="https://img.shields.io/discord/853833144037277726?style=plastic" alt="Arrow DAO Discord">
//!     </a>
//!     <br><br>
//! </div>
//!
//! Exposes svc-telemetry grpc client functions

/// Client Library: Client Functions, Structs
pub mod client {
    #![allow(unused_qualifications)]
    include!("grpc.rs");
}
