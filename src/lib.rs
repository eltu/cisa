//! Search autocomplete server exposing one session protocol over gRPC and WebSocket.

pub mod autocomplete;
pub mod config;
pub mod shutdown;
pub mod transport;

/// Code generated from `proto/cisa/v1/autocomplete.proto`.
pub mod proto {
    #![allow(
        clippy::all,
        clippy::pedantic,
        clippy::restriction,
        missing_debug_implementations,
        reason = "generated code is not held to project lint rules"
    )]
    tonic::include_proto!("cisa.v1");
}
