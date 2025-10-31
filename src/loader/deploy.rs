use super::write_buffer;
use crate::pw::whitelist::require_whitelist_entry;
use eyre::{eyre, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
#[allow(deprecated)]
use spherenet_whitelisted_loader_v3_interface::{
    instruction::{create_buffer, deploy_with_max_program_len},
    state::UpgradeableLoaderState,
};
use std::fs;

/// Deploy a program to SphereNet
pub fn deploy(
    url: &str,
    program_so_path: String,
    program_keypair_path: String,
    upgrade_authority_str: String,
    payer_keypair_path: String,
    max_data_len: Option<usize>,
) -> Result<()> {
    println!("🚀 Deploying program to SphereNet...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Load keypairs
    let payer = read_keypair_file(&payer_keypair_path)
        .map_err(|e| eyre!("Failed to read payer keypair from {}: {}", payer_keypair_path, e))?;
    let program_keypair = read_keypair_file(&program_keypair_path)
        .map_err(|e| eyre!("Failed to read program keypair from {}: {}", program_keypair_path, e))?;
    let program_id = program_keypair.pubkey();
    let upgrade_authority_keypair = read_keypair_file(&upgrade_authority_str)
        .map_err(|e| eyre!("Failed to read upgrade authority keypair from {}: {}", upgrade_authority_str, e))?;
    let upgrade_authority = upgrade_authority_keypair.pubkey();

    println!("  Program ID: {}", program_id);
    println!("  Payer: {}", payer.pubkey());
    println!("  Upgrade Authority: {}", upgrade_authority);

    // Read program .so file
    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    println!("  Program size: {} bytes", program_data.len());

    // Determine max data length
    let max_data_len = max_data_len.unwrap_or(program_data.len());
    if max_data_len < program_data.len() {
        return Err(eyre!(
            "max_data_len ({}) must be at least program size ({})",
            max_data_len,
            program_data.len()
        ));
    }

    // Verify upgrade authority is whitelisted before spending lamports (fail-fast)
    let whitelist_entry = require_whitelist_entry(&rpc_client, upgrade_authority)?;

    // Create and write buffer
    println!("\n📝 Creating buffer account...");
    let buffer_keypair = Keypair::new();
    let buffer_pubkey = buffer_keypair.pubkey();
    let buffer_size = UpgradeableLoaderState::size_of_buffer(program_data.len());
    let buffer_lamports = rpc_client.get_minimum_balance_for_rent_exemption(buffer_size)?;

    println!("  Buffer size: {} bytes", buffer_size);
    println!("  Buffer rent: {} lamports", buffer_lamports);

    // Create and initialize buffer account with UPGRADE AUTHORITY as authority
    // (required for deployment - buffer authority must match program's upgrade authority)
    let create_buffer_instructions = create_buffer(
        &payer.pubkey(),
        &buffer_pubkey,
        &upgrade_authority, // Must match the program's upgrade authority
        buffer_lamports,
        program_data.len(),
    )?;

    let mut transaction =
        Transaction::new_with_payer(&create_buffer_instructions, Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &buffer_keypair],
        rpc_client.get_latest_blockhash()?,
    );
    rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("  ✓ Buffer account created: {}", buffer_pubkey);

    // Write program data to buffer in chunks (upgrade authority signs as buffer authority)
    println!("\n📤 Writing program data to buffer...");
    write_buffer(
        &rpc_client,
        &payer,
        &upgrade_authority_keypair,
        &buffer_pubkey,
        &upgrade_authority,
        &program_data,
    )?;

    // Deploy program with whitelist validation
    println!("\n🎯 Deploying program...");
    let program_lamports = rpc_client
        .get_minimum_balance_for_rent_exemption(UpgradeableLoaderState::size_of_program())?;

    #[allow(deprecated)]
    let deploy_instructions = deploy_with_max_program_len(
        &payer.pubkey(),
        &program_id,
        &buffer_pubkey,
        &upgrade_authority,
        &whitelist_entry,
        program_lamports,
        max_data_len,
    )?;

    let mut transaction = Transaction::new_with_payer(&deploy_instructions, Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &program_keypair, &upgrade_authority_keypair],
        rpc_client.get_latest_blockhash()?,
    );
    rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Program deployed successfully!");
    println!("   Program ID: {}", program_id);
    println!("   Upgrade Authority: {}", upgrade_authority);

    Ok(())
}
