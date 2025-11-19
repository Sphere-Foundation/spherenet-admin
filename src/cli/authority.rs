//! Authority management module
//!
//! Unified authority handling for single-sig and multi-sig execution.

use crate::squads;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use std::str::FromStr;

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
    ) -> eyre::Result<Self> {
        match (authority, vault, multisig_authority) {
            (Some(authority_path), None, None) => {
                // Single-sig mode
                let keypair =
                    solana_sdk::signature::read_keypair_file(&authority_path).map_err(|e| {
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
                let signer =
                    solana_sdk::signature::read_keypair_file(&signer_path).map_err(|e| {
                        eyre::eyre!(
                            "Failed to load multisig authority keypair from {}: {}",
                            signer_path,
                            e
                        )
                    })?;
                Ok(Authority::MultiSig { vault, signer })
            }
            _ => Err(eyre::eyre!(
                "Must provide either --authority OR (--multisig + --multisig-authority)"
            )),
        }
    }

    /// Get the pubkey to use as "from" in instructions (the actual vault that holds funds)
    ///
    /// For SingleSig: returns the keypair pubkey
    /// For MultiSig: returns the vault PDA (derived from multisig with vault_index=0)
    pub fn instruction_authority_pubkey(&self) -> eyre::Result<Pubkey> {
        match self {
            Authority::SingleSig { keypair } => Ok(keypair.pubkey()),
            Authority::MultiSig { vault, .. } => {
                let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
                let (vault_pda, _) = squads::types::get_vault_pda(vault, 0, &program_id);
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
    /// - `ExecutionResult::Executed` for single-sig (with signature)
    /// - `ExecutionResult::ProposalCreated` for multi-sig (with proposal address)
    pub fn execute_instruction(
        &self,
        rpc: &RpcClient,
        instruction: Instruction,
        description: &str,
    ) -> eyre::Result<ExecutionResult> {
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
                println!("\nCreating proposal: {}", description);

                // Parse program ID
                let program_id = Pubkey::from_str(squads::types::SQUADS_PROGRAM_ID)
                    .map_err(|e| eyre::eyre!("Invalid SQUADS_PROGRAM_ID: {}", e))?;

                // Fetch multisig account to get next transaction_index
                let multisig_account = rpc.get_account(vault).map_err(|e| {
                    eyre::eyre!("Failed to fetch multisig account at {}: {}", vault, e)
                })?;
                let current_transaction_index = squads::types::parse_transaction_index(&multisig_account.data)?;
                let next_transaction_index = current_transaction_index.checked_add(1)
                    .ok_or_else(|| eyre::eyre!("Transaction index overflow"))?;

                println!("   Transaction Index: {}", next_transaction_index);

                // Derive PDAs (these functions handle the +1 internally)
                let (vault_transaction_pda, _) =
                    squads::types::get_vault_transaction_pda(vault, current_transaction_index, &program_id);
                let (proposal_pda, _) =
                    squads::types::get_proposal_pda(vault, current_transaction_index, &program_id);

                // Derive the actual vault PDA (where SOL is held and that signs transactions)
                let (vault_pda, _) = squads::types::get_vault_pda(vault, 0, &program_id); // vault_index = 0

                // Compile instruction into Squads TransactionMessage format
                let transaction_message = squads::types::compile_instruction_to_transaction_message(
                    &instruction,
                    &vault_pda, // Use vault PDA, not multisig PDA
                );

                // Serialize to bytes using Borsh
                let transaction_message_bytes = borsh::to_vec(&transaction_message)
                    .map_err(|e| eyre::eyre!("Failed to serialize transaction message: {}", e))?;

                // Build vault_transaction_create instruction
                let vault_tx_args = squads::types::VaultTransactionCreateArgs {
                    vault_index: 0,           // Default vault
                    ephemeral_signers: 0,     // No ephemeral signers
                    transaction_message: transaction_message_bytes,
                    memo: Some(description.to_string()),
                };

                let vault_tx_create_ix = squads::instructions::build_vault_transaction_create_ix(
                    &program_id,
                    vault,
                    &vault_transaction_pda,
                    &signer.pubkey(),
                    &signer.pubkey(), // rent_payer
                    vault_tx_args,
                )?;

                // Build proposal_create instruction
                let proposal_args = squads::types::ProposalCreateArgs {
                    transaction_index: next_transaction_index,
                    draft: false, // Active proposal (ready for voting)
                };

                let proposal_create_ix = squads::instructions::build_proposal_create_ix(
                    &program_id,
                    vault,
                    &proposal_pda,
                    &signer.pubkey(),
                    &signer.pubkey(), // rent_payer
                    proposal_args,
                )?;

                // Send both instructions in one transaction
                let recent_blockhash = rpc.get_latest_blockhash()?;
                let tx = Transaction::new_signed_with_payer(
                    &[vault_tx_create_ix, proposal_create_ix],
                    Some(&signer.pubkey()),
                    &[signer],
                    recent_blockhash,
                );

                let signature = rpc.send_and_confirm_transaction(&tx)?;

                println!("\n✅ Proposal created successfully!");
                println!("   Signature: {}", signature);
                println!("   Proposal:  {}", proposal_pda);

                Ok(ExecutionResult::ProposalCreated {
                    proposal: proposal_pda,
                    transaction_index: next_transaction_index,
                })
            }
        }
    }
}
