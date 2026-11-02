//! Application-owned composite-context and atomic state boundary for AMRI.
//!
//! This module is deliberately outside `crate::amri`. The Stage 2 protocol Core remains
//! pure and store independent; no FACTSTR types cross this boundary's public port.

pub mod context;
pub mod factstr;
pub mod state_port;

#[cfg(test)]
mod fake;
