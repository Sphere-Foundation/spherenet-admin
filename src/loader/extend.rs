use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spherenet_authority::Authority;
use std::str::FromStr;

/// Extend a program's data account to accommodate larger programs
///
/// Uses ExtendProgramChecked instruction which is CPI-safe and works with multisig.
///
/// Note: The payer parameter is accepted but not used for the extend instruction itself.
/// The vault PDA (instruction_authority) pays for the rent increase via invoke_signed.
pub fn extend_program(
    url: &str,
    program_id_str: String,
    additional_bytes: u32,
    upgrade_authority: Authority,
    _payer_keypair_path: String,
) -> eyre::Result<()> {
    println!("🔧 Extending program data account...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Parse program ID
    let program_id = Pubkey::from_str(&program_id_str)
        .map_err(|e| eyre::eyre!("Failed to parse program ID {}: {}", program_id_str, e))?;

    // Get instruction authority (vault PDA for multisig, keypair pubkey for single-sig)
    let instruction_authority = upgrade_authority.instruction_authority_pubkey()?;

    println!("  Program ID:         {}", program_id);
    println!("  Additional Bytes:   {}", additional_bytes);
    println!("  Upgrade Authority:  {}", instruction_authority);
    println!("  Payer:              {}", instruction_authority);

    // Build extend_program_checked instruction (CPI-safe, works with multisig!)
    // Note: Payer must be instruction_authority (vault PDA) for multisig so vault can pay
    let extend_ix = spherenet_whitelisted_loader_v3_interface::instruction::extend_program_checked(
        &program_id,
        &instruction_authority, // Authority required - vault PDA for multisig
        Some(&instruction_authority), // Payer also vault PDA for multisig consistency
        additional_bytes,
    );

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Extend program {} by {} bytes",
        program_id, additional_bytes
    );
    let result = upgrade_authority.execute_instruction(&rpc_client, extend_ix, &description)?;

    // Only show "extended successfully" for single-sig (immediate execution)
    if matches!(
        result,
        spherenet_authority::ExecutionResult::Executed { .. }
    ) {
        println!("\n✅ Program data account extended successfully!");
    }

    Ok(())
}
