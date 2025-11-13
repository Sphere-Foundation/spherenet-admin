use crate::cli::{authority::ExecutionResult, Authority};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    signature::Signer,
};
use std::str::FromStr;

#[allow(deprecated)]
use solana_sdk::system_instruction;

/// Transfer SOL from one account to another
pub fn transfer(
    rpc_url: &str,
    from: Authority,
    destination_str: String,
    amount: f64,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Parse destination pubkey
    let destination = Pubkey::from_str(&destination_str).map_err(|e| {
        eyre::eyre!(
            "Failed to parse destination pubkey {}: {}",
            destination_str,
            e
        )
    })?;

    // Convert SOL to lamports
    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    println!("\nTransferring {} SOL to {}", amount, destination);

    // Display different info based on authority type
    match &from {
        crate::cli::Authority::SingleSig { keypair } => {
            println!("  From:        {}", keypair.pubkey());
        }
        crate::cli::Authority::MultiSig { vault, signer } => {
            println!("  Proposer:    {}", signer.pubkey());
            println!("  Multisig:    {}", vault);

            // Derive and show vault PDA (where funds will come from)
            let program_id =
                crate::squads::types::SQUADS_PROGRAM_ID.parse::<solana_sdk::pubkey::Pubkey>()?;
            let (vault_pda, _) = crate::squads::types::get_vault_pda(&vault, 0, &program_id);

            // Get vault balance
            let vault_balance = rpc_client.get_balance(&vault_pda).unwrap_or(0);
            let vault_balance_sol = vault_balance as f64 / LAMPORTS_PER_SOL as f64;

            println!("  Vault PDA:   {}", vault_pda);
            println!("    Balance:   {:.9} SOL", vault_balance_sol);
        }
    }
    println!();

    // Build system transfer instruction
    // For multisig, we need the vault PDA (not the multisig PDA) as the "from" address
    let from_pubkey = from.instruction_authority_pubkey()?;
    let instruction = system_instruction::transfer(&from_pubkey, &destination, lamports);

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Transfer {} SOL to {}", amount, destination);
    let result = from.execute_instruction(&rpc_client, instruction, &description)?;

    match result {
        ExecutionResult::Executed { signature } => {
            println!("\n✅ Transfer executed successfully!");
            println!("   Signature: {}", signature);
        }
        ExecutionResult::ProposalCreated {
            proposal,
            transaction_index,
        } => {
            println!("\n✅ Multisig proposal created!");
            println!("   Proposal:          {}", proposal);
            println!("   Transaction Index: {}", transaction_index);
            println!("\nNext steps:");
            println!("  1. Vault members approve: spherenet-admin multisig approve ...");
            println!("  2. Execute proposal:      spherenet-admin multisig execute ...");
        }
    }

    Ok(())
}
