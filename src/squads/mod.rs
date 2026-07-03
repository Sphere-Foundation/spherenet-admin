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
//! - ixs.rs: Instruction builders (create, approve, execute)
//! - show.rs: multisig read commands
//! - run.rs: multisig write commands (create, approve, execute)

pub mod ixs;
pub mod run;
pub mod show;
pub mod types;

// Re-export commonly used types
pub use types::{Member, Multisig, Permissions};

use borsh::BorshDeserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

/// A create-key resolved against the chain: the derived PDAs plus the on-chain
/// account. Producing one is proof the multisig actually exists — every command
/// goes through [`resolve`], so callers never handle a raw PDA and can pick the
/// right one for the job (funds → `vault`, config/signing → `multisig`).
pub struct ResolvedMultisig {
    /// Config account PDA (members, threshold) — what instructions act on.
    pub multisig: Pubkey,
    /// Vault PDA (index 0) — holds funds and signs wrapped instructions.
    pub vault: Pubkey,
    /// Deserialized on-chain config account (a pure borsh mirror).
    pub account: Multisig,
}

/// Resolve and validate a multisig by its create-key.
///
/// Derives the multisig + vault PDAs, fetches the config account, and verifies
/// it is a real Squads multisig for this create-key. Fails fast (rather than
/// silently deriving addresses for a nonexistent multisig) when the create-key
/// is bogus — so downstream commands can trust the returned PDAs.
pub fn resolve(rpc: &RpcClient, create_key: &Pubkey) -> eyre::Result<ResolvedMultisig> {
    let program_id = types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
    let (multisig, _) = types::get_multisig_pda(create_key, &program_id);
    let (vault, _) = types::get_vault_pda(&multisig, 0, &program_id);

    let account = rpc.get_account(&multisig).map_err(|_| {
        eyre::eyre!("No multisig found for create-key {create_key} (derived PDA {multisig})")
    })?;

    if account.owner != program_id {
        return Err(eyre::eyre!(
            "Account {multisig} is not a Squads multisig (owner {})",
            account.owner
        ));
    }

    // Deserialize (skip the 8-byte Anchor discriminator; tolerate extra space).
    if account.data.len() < 8 {
        return Err(eyre::eyre!("Invalid multisig account data (too short)"));
    }
    let mut data = &account.data[8..];
    let parsed = Multisig::deserialize_reader(&mut data)
        .map_err(|e| eyre::eyre!("Failed to deserialize multisig account: {e}"))?;

    // The account self-reports its create-key — the definitive "is this real" check.
    if parsed.create_key != *create_key {
        return Err(eyre::eyre!(
            "Account {multisig} create_key mismatch (expected {create_key}, found {})",
            parsed.create_key
        ));
    }

    Ok(ResolvedMultisig {
        multisig,
        vault,
        account: parsed,
    })
}

/// How a pubkey relates to Squads — for callers that must avoid mis-sending
/// funds to a multisig. `None` = not multisig-related (a plain wallet, or a
/// vault PDA, which is a safe destination).
pub enum MultisigRef {
    /// A Squads-owned account (e.g. a multisig config account) — never a valid
    /// fund destination.
    ConfigAccount,
    /// The create-key of an existing multisig — the vault, not this account, is
    /// the intended destination.
    CreateKey,
}

/// Classify a pubkey's relationship to Squads, if any. The detection lives here
/// (squads domain knowledge); callers decide the policy.
pub fn classify(rpc: &RpcClient, pubkey: &Pubkey) -> eyre::Result<Option<MultisigRef>> {
    let program_id = types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // A Squads-owned account (config/state) — never a plain wallet or vault.
    if let Ok(account) = rpc.get_account(pubkey) {
        if account.owner == program_id {
            return Ok(Some(MultisigRef::ConfigAccount));
        }
    }

    // Resolves as a create-key → it identifies a real multisig.
    if resolve(rpc, pubkey).is_ok() {
        return Ok(Some(MultisigRef::CreateKey));
    }

    Ok(None)
}
