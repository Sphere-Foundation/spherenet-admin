//! CLI infrastructure module
//!
//! Contains command-line interface structures, routing, and execution logic.

pub mod authority_builder;
pub mod commands;
pub mod output;
pub mod run;

pub use run::run;
