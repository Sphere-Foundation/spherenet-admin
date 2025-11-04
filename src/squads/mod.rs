//! Squads v4 SDK-free integration
//!
//! Manual instruction building for Squads v4 without SDK dependencies.
//!
//! **Why manual instructions?**
//! The squads-multisig SDK has broken dependencies:
//! - squads-multisig-program uses Solana 1.17.4 + Anchor 0.29.0 (ancient)
//! - Published SDK requires Solana 2.3.x
//! - Dependency tree conflicts are unresolvable
//!
//! **Solution:**
//! Build instructions manually by extracting types from program source
//! and using Anchor discriminator calculation + PDA derivation.
//!
//! **Structure:**
//! - types.rs: Type definitions, constants, PDA helpers
//! - instructions.rs: Instruction builders (create, approve, execute)

pub mod instructions;
pub mod types;

// Re-export commonly used types
pub use types::{Member, Permissions, ProgramConfigInitArgs};
