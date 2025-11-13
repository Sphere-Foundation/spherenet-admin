use crate::cli::{authority::ExecutionResult, Authority};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
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

    println!("\nTransferring SOL:");
    println!("  From:        {}", from.pubkey());
    println!("  Destination: {}", destination);
    println!("  Amount:      {} SOL ({} lamports)", amount, lamports);

    // Build system transfer instruction
    let instruction = system_instruction::transfer(&from.pubkey(), &destination, lamports);

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
