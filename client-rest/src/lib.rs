#![doc = include_str!("../README.md")]

/// Types for NETRID packets (temporary)
///  TODO(R5): Move NETRID types to a separate crate
pub mod netrid_types {
    include!("../../server/src/msg/netrid.rs");
}

/// Types for ADSB packets (temporary)
//  TODO(R5): Move ADSB types to a separate crate
pub mod adsb_types {
    include!("../../server/src/msg/adsb.rs");
}
