//! Authority management
//!
//! Unified authority handling for single-sig and multi-sig execution, plus the
//! CLI-arg constructor ([`from_cli_args`]). Uses the top-level `squads` client
//! for the multisig proposal path.

use crate::cli::output::{progress, TxOutputView};
use crate::squads;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    transaction::Transaction,
};

/// Authority that can execute instructions
pub enum Authority {
    /// Single-signature authority (direct execution)
    SingleSig { keypair: Keypair },
    /// Multi-signature authority (creates proposals)
    MultiSig { multisig: Pubkey, member: Keypair },
}

impl Authority {
    /// Get the pubkey to use as "from" in instructions (the actual vault that holds funds)
    ///
    /// For SingleSig: returns the keypair pubkey
    /// For MultiSig: returns the vault PDA (derived from multisig with vault_index=0)
    pub fn instruction_authority_pubkey(&self) -> eyre::Result<Pubkey> {
        match self {
            Authority::SingleSig { keypair } => Ok(keypair.pubkey()),
            Authority::MultiSig { multisig, .. } => {
                let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
                let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);
                Ok(vault_pda)
            }
        }
    }

    /// Execute an instruction using this authority
    ///
    /// # Arguments
    /// * `rpc` - RPC client for blockchain interaction
    /// * `instruction` - The instruction to execute or propose
    /// * `description` - Human-readable description for logging
    ///
    /// # Returns
    /// - `TxOutputView::Executed` for single-sig (with signature)
    /// - `TxOutputView::ProposalCreated` for multi-sig (with proposal address)
    pub fn execute_instruction(
        &self,
        rpc: &RpcClient,
        instruction: Instruction,
        description: &str,
    ) -> eyre::Result<TxOutputView> {
        match self {
            Authority::SingleSig { keypair } => {
                progress(format!("Executing: {}", description));

                // Build and send transaction directly
                let recent_blockhash = rpc.get_latest_blockhash()?;
                let tx = Transaction::new_signed_with_payer(
                    &[instruction],
                    Some(&keypair.pubkey()),
                    &[keypair],
                    recent_blockhash,
                );
                let signature = rpc.send_and_confirm_transaction(&tx)?;

                Ok(TxOutputView::Executed {
                    signature: signature.to_string(),
                })
            }
            // Delegate the entire proposal flow to the squads client — the
            // adapter knows nothing about Squads PDAs or wire formats.
            Authority::MultiSig { multisig, member } => {
                squads::run::propose(rpc, multisig, member, instruction, description)
            }
        }
    }
}

/// Build an [`Authority`] from CLI arguments.
///
/// # Arguments
/// * `authority` - Single-sig: path to authority keypair
/// * `multisig` - Multi-sig: multisig PDA address
/// * `multisig_authority` - Multi-sig: path to member keypair
///
/// # Returns
/// - `Authority::SingleSig` if only `authority` is provided
/// - `Authority::MultiSig` if `multisig` and `multisig_authority` are provided
/// - Error if neither or an invalid combination is provided
pub fn from_cli_args(
    authority: Option<String>,
    multisig: Option<String>,
    multisig_authority: Option<String>,
) -> eyre::Result<Authority> {
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
