//! `rekall` -- turn always-on agent prose into tangibles.
//!
//! The library half of the crate. The binary is a thin caller: everything
//! it does is reachable here, so a consumer can drive the same operations
//! without shelling out and a test can assert on values rather than on
//! formatted output.

pub mod config;
pub mod corpus;
pub mod statement;
