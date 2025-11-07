//! Authority management module
//!
//! Unified authority handling for single-sig and multi-sig execution.

use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};

/// Result of executing an instruction through Authority
#[derive(Debug)]
pub enum ExecutionResult {
    /// Instruction was executed directly (single-sig)
    Executed { signature: Signature },
    /// Proposal was created for multisig approval
    ProposalCreated {
        proposal: Pubkey,
        transaction_index: u64,
    },
}

/// Authority that can execute instructions
pub enum Authority {
    /// Single-signature authority (direct execution)
    SingleSig { keypair: Keypair },
    /// Multi-signature authority (creates proposals)
    MultiSig { vault: Pubkey, signer: Keypair },
}

impl Authority {
    /// Parse Authority from CLI arguments
    ///
    /// # Arguments
    /// * `authority` - Single-sig: path to authority keypair
    /// * `vault` - Multi-sig: vault address
    /// * `multisig_authority` - Multi-sig: path to signer keypair
    ///
    /// # Returns
    /// - `Authority::SingleSig` if only `authority` is provided
    /// - `Authority::MultiSig` if `vault` and `multisig_authority` are provided
    /// - Error if neither or invalid combination is provided
    pub fn from_cli_args(
        authority: Option<String>,
        vault: Option<String>,
        multisig_authority: Option<String>,
    ) -> Result<Self> {
        match (authority, vault, multisig_authority) {
            (Some(authority_path), None, None) => {
                // Single-sig mode
                let keypair = solana_sdk::signature::read_keypair_file(&authority_path)
                    .map_err(|e| {
                        eyre::eyre!(
                            "Failed to load authority keypair from {}: {}",
                            authority_path,
                            e
                        )
                    })?;
                Ok(Authority::SingleSig { keypair })
            }
            (None, Some(vault_str), Some(signer_path)) => {
                // Multi-sig mode
                let vault = vault_str
                    .parse::<Pubkey>()
                    .map_err(|e| eyre::eyre!("Invalid vault address '{}': {}", vault_str, e))?;
                let signer = solana_sdk::signature::read_keypair_file(&signer_path)
                    .map_err(|e| {
                        eyre::eyre!(
                            "Failed to load multisig authority keypair from {}: {}",
                            signer_path,
                            e
                        )
                    })?;
                Ok(Authority::MultiSig { vault, signer })
            }
            _ => Err(eyre::eyre!(
                "Must provide either --authority OR (--vault + --multisig-authority)"
            )),
        }
    }

    /// Get the pubkey of the signer/payer
    pub fn pubkey(&self) -> Pubkey {
        match self {
            Authority::SingleSig { keypair } => keypair.pubkey(),
            Authority::MultiSig { signer, .. } => signer.pubkey(),
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
    /// - `ExecutionResult::Executed` for single-sig (with signature)
    /// - `ExecutionResult::ProposalCreated` for multi-sig (with proposal address)
    pub fn execute_instruction(
        &self,
        rpc: &RpcClient,
        instruction: Instruction,
        description: &str,
    ) -> Result<ExecutionResult> {
        match self {
            Authority::SingleSig { keypair } => {
                println!("Executing: {}", description);
                println!();

                // Build and send transaction directly
                let recent_blockhash = rpc.get_latest_blockhash()?;
                let tx = Transaction::new_signed_with_payer(
                    &[instruction],
                    Some(&keypair.pubkey()),
                    &[keypair],
                    recent_blockhash,
                );
                let signature = rpc.send_and_confirm_transaction(&tx)?;

                println!("✅ Transaction executed successfully!");
                println!("   Signature: {}", signature);
                println!();

                Ok(ExecutionResult::Executed { signature })
            }
            Authority::MultiSig { vault, signer } => {
                println!("Creating proposal: {}", description);
                println!("   Vault: {}", vault);
                println!();

                // TODO: Implement multisig proposal creation
                // 1. Fetch multisig account to get next transaction_index
                // 2. Serialize instruction into TransactionMessage
                // 3. Create vault_transaction
                // 4. Create proposal
                // 5. Return proposal info

                todo!("Implement multisig proposal creation")
            }
        }
    }
}
