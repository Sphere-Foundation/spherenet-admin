//! Feature-gate commands: `activate` / `revoke` (mutations) and `status` (read).
//!
//! A gate is a Feature-program account at the gate's own pubkey holding
//! `Feature { activated_at: Option<u64> }`. Absent = old path; `None` = staged
//! (pending); `Some(slot)` = active. Staging is signed by the gate keypair; the
//! runtime flips pending → active at the next epoch boundary. See the shared
//! parse helpers below, used by both `run` and `show`.

pub mod run;
pub mod show;

use solana_feature_gate_interface::state::Feature;
use solana_sdk::{account::Account, pubkey::Pubkey};

/// The native Feature program id, byte-normalized to the repo's `Pubkey` type.
pub(crate) fn feature_program_id() -> Pubkey {
    Pubkey::from(solana_feature_gate_interface::id().to_bytes())
}

/// Parse a fetched account as a `Feature`: `None` unless it is owned by the
/// Feature program and at least `Feature::size_of()` bytes. Mirrors the crate's
/// `from_account`, but over the repo's `Account` type — avoiding a cross-crate
/// `ReadableAccount` bound, the same manual approach the stake module uses.
pub(crate) fn parse_feature(account: &Account) -> Option<Feature> {
    if account.owner != feature_program_id() || account.data.len() < Feature::size_of() {
        None
    } else {
        bincode::deserialize(&account.data).ok()
    }
}
