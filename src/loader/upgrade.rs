use super::write_buffer;
use crate::pw::whitelist::verify_whitelist_authority;
use eyre::{eyre, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
#[allow(deprecated)]
use spherenet_whitelisted_loader_v3_interface::{
    instruction::{create_buffer, upgrade},
    state::UpgradeableLoaderState,
};
use std::{fs, str::FromStr};

/// Upgrade an existing program on SphereNet
pub fn upgrade_program(
    url: &str,
    program_id_str: String,
    program_so_path: String,
    upgrade_authority_str: String,
    payer_keypair_path: String,
    spill_address_str: Option<String>,
) -> Result<()> {
    println!("🔄 Upgrading program on SphereNet...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Load keypairs and parse addresses
    let payer = read_keypair_file(&payer_keypair_path)
        .map_err(|e| eyre!("Failed to read payer keypair from {}: {}", payer_keypair_path, e))?;
    let upgrade_authority_keypair = read_keypair_file(&upgrade_authority_str)
        .map_err(|e| eyre!("Failed to read upgrade authority keypair from {}: {}", upgrade_authority_str, e))?;
    let upgrade_authority = upgrade_authority_keypair.pubkey();
    let program_id = Pubkey::from_str(&program_id_str)
        .map_err(|e| eyre!("Failed to parse program ID {}: {}", program_id_str, e))?;

    // Spill account defaults to payer if not specified
    let spill_address = if let Some(spill_str) = spill_address_str {
        Pubkey::from_str(&spill_str)
            .map_err(|e| eyre!("Failed to parse spill address {}: {}", spill_str, e))?
    } else {
        payer.pubkey()
    };

    println!("  Program ID: {}", program_id);
    println!("  Payer: {}", payer.pubkey());
    println!("  Upgrade Authority: {}", upgrade_authority);
    println!("  Spill Account: {}", spill_address);

    // Verify program exists
    match rpc_client.get_account(&program_id) {
        Ok(_) => println!("  ✓ Program exists"),
        Err(_) => {
            return Err(eyre!(
                "❌ Program {} does not exist! Use 'program deploy' to deploy a new program.",
                program_id
            ));
        }
    }

    // Read program .so file
    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    println!("  New program size: {} bytes", program_data.len());

    // Verify upgrade authority is whitelisted before spending lamports (fail-fast)
    let whitelist_entry = verify_whitelist_authority(&rpc_client, upgrade_authority)?;

    // Create and write buffer
    println!("\n📝 Creating buffer account...");
    let buffer_keypair = Keypair::new();
    let buffer_pubkey = buffer_keypair.pubkey();
    let buffer_size = UpgradeableLoaderState::size_of_buffer(program_data.len());
    let buffer_lamports = rpc_client.get_minimum_balance_for_rent_exemption(buffer_size)?;

    println!("  Buffer size: {} bytes", buffer_size);
    println!("  Buffer rent: {} lamports", buffer_lamports);

    // Create and initialize buffer account with UPGRADE AUTHORITY as authority
    // (required for upgrade - buffer authority must match program's upgrade authority)
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

    // Upgrade program with whitelist validation
    println!("\n🎯 Upgrading program...");

    #[allow(deprecated)]
    let upgrade_ix = upgrade(
        &program_id,
        &buffer_pubkey,
        &upgrade_authority,
        &spill_address,
        &whitelist_entry,
    );

    let mut transaction = Transaction::new_with_payer(&[upgrade_ix], Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &upgrade_authority_keypair],
        rpc_client.get_latest_blockhash()?,
    );
    rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Program upgraded successfully!");
    println!("   Program ID: {}", program_id);
    println!("   Upgrade Authority: {}", upgrade_authority);

    Ok(())
}
