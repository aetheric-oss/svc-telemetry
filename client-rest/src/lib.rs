#![doc = include_str!("../README.md")]

/// Types for API messages
pub mod types {
    include!("../../openapi/types.rs");
}

/// Types for NETRID packages (temporary)
///  TODO(R5): Move NETRID types to a separate crate
pub mod netrid_types {
    include!("../../server/src/msg/netrid.rs");
}
