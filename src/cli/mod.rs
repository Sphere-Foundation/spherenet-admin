//! CLI infrastructure module
//!
//! Contains command-line interface structures, routing, and execution logic.

pub mod authority;
pub mod commands;
pub mod run;

pub use authority::Authority;
pub use run::run;
