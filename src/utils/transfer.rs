use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    signature::Signer,
};
use spherenet_authority::{squads, Authority, ExecutionResult};
use std::str::FromStr;

use solana_system_interface::instruction as system_instruction;

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
        spherenet_authority::Authority::SingleSig { keypair } => {
            println!("  From:        {}", keypair.pubkey());
        }
        spherenet_authority::Authority::MultiSig { multisig, member } => {
            println!("  Proposer:    {}", member.pubkey());
            println!("  Multisig:    {}", multisig);

            // Derive and show vault PDA (where funds will come from)
            let program_id =
                squads::types::SQUADS_PROGRAM_ID.parse::<solana_sdk::pubkey::Pubkey>()?;
            let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);

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

    // Only show "executed successfully" for single-sig (immediate execution)
    if matches!(result, ExecutionResult::Executed { .. }) {
        println!("\n✅ Transfer executed successfully!");
    }

    Ok(())
}
