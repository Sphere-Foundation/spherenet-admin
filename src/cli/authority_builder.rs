//! CLI-specific authority construction helpers

use eyre::Result;
use solana_sdk::{pubkey::Pubkey, signature::read_keypair_file};
use crate::authority::Authority;

/// Build Authority from CLI arguments
///
/// # Arguments
/// * `authority` - Single-sig: path to authority keypair
/// * `multisig` - Multi-sig: multisig PDA address
/// * `multisig_authority` - Multi-sig: path to member keypair
///
/// # Returns
/// - `Authority::SingleSig` if only `authority` is provided
/// - `Authority::MultiSig` if `multisig` and `multisig_authority` are provided
/// - Error if neither or invalid combination is provided
pub fn from_cli_args(
    authority: Option<String>,
    multisig: Option<String>,
    multisig_authority: Option<String>,
) -> Result<Authority> {
    match (authority, multisig, multisig_authority) {
        (Some(authority_path), None, None) => {
            // Single-sig mode
            let keypair = read_keypair_file(&authority_path).map_err(|e| {
                eyre::eyre!(
                    "Failed to load authority keypair from {}: {}",
                    authority_path,
                    e
                )
            })?;
            Ok(Authority::SingleSig { keypair })
        }
        (None, Some(multisig_str), Some(member_path)) => {
            // Multi-sig mode
            let multisig = multisig_str
                .parse::<Pubkey>()
                .map_err(|e| eyre::eyre!("Invalid multisig address '{}': {}", multisig_str, e))?;
            let member = read_keypair_file(&member_path).map_err(|e| {
                eyre::eyre!(
                    "Failed to load multisig member keypair from {}: {}",
                    member_path,
                    e
                )
            })?;
            Ok(Authority::MultiSig { multisig, member })
        }
        _ => Err(eyre::eyre!(
            "Must provide either --authority OR (--multisig + --multisig-authority)"
        )),
    }
}
