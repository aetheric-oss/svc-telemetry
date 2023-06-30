//! API

pub mod aircraft;
pub mod health;
pub mod mavlink;

/// Types Used in REST Messages
pub mod rest_types {
    include!("../../../../openapi/types.rs");
}

pub use rest_types::Keys;
