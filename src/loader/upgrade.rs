use super::write_buffer;
use crate::pw::whitelist::require_whitelist_entry;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use spherenet_authority::Authority;
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
    upgrade_authority: Authority,
    payer_keypair_path: String,
    spill_address_str: Option<String>,
) -> eyre::Result<()> {
    println!("🔄 Upgrading program on SphereNet...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Load keypairs and parse addresses
    let payer = read_keypair_file(&payer_keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read payer keypair from {}: {}",
            payer_keypair_path,
            e
        )
    })?;
    let program_id = Pubkey::from_str(&program_id_str)
        .map_err(|e| eyre::eyre!("Failed to parse program ID {}: {}", program_id_str, e))?;

    // Spill account defaults to payer if not specified
    let spill_address = if let Some(spill_str) = spill_address_str {
        Pubkey::from_str(&spill_str)
            .map_err(|e| eyre::eyre!("Failed to parse spill address {}: {}", spill_str, e))?
    } else {
        payer.pubkey()
    };

    // Get instruction authority early for display and later use
    let instruction_authority = upgrade_authority.instruction_authority_pubkey()?;

    println!("  Program ID: {}", program_id);
    println!("  Payer: {}", payer.pubkey());
    println!("  Upgrade Authority: {}", instruction_authority);
    println!("  Spill Account: {}", spill_address);

    // Verify program exists
    match rpc_client.get_account(&program_id) {
        Ok(_) => println!("  ✓ Program exists"),
        Err(_) => {
            return Err(eyre::eyre!(
                "❌ Program {} does not exist! Use 'program deploy' to deploy a new program.",
                program_id
            ));
        }
    }

    // Read program .so file
    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre::eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    println!("  New program size: {} bytes", program_data.len());

    // Verify program data account is large enough for the new program (fail-fast)
    println!("\n🔍 Checking program capacity...");
    let (programdata_address, _) = Pubkey::find_program_address(
        &[program_id.as_ref()],
        &solana_sdk::bpf_loader_upgradeable::id(),
    );

    let programdata_account = rpc_client.get_account(&programdata_address).map_err(|e| {
        eyre::eyre!(
            "Failed to get ProgramData account {}: {}",
            programdata_address,
            e
        )
    })?;

    // Parse ProgramData account to get max_data_len
    // ProgramData layout: [account_type: 4 bytes][slot: 8 bytes][upgrade_authority: 32 bytes][actual_data...]
    // For upgradeable programs, the max_data_len is the total account size minus the metadata overhead (45 bytes)
    let programdata_metadata_len = 45; // Account type (4) + slot (8) + authority (32) + reserved (1)
    let current_max_len = programdata_account
        .data
        .len()
        .saturating_sub(programdata_metadata_len);

    println!("  Current max capacity: {} bytes", current_max_len);
    println!("  Required capacity:    {} bytes", program_data.len());

    if program_data.len() > current_max_len {
        let additional_bytes = program_data.len() - current_max_len;

        // Build the appropriate extend command based on authority type
        let extend_cmd = match &upgrade_authority {
            spherenet_authority::Authority::SingleSig { .. } => {
                format!(
                    "spherenet-admin program extend \\\n  \
                    --program-id {} \\\n  \
                    --bytes {} \\\n  \
                    --upgrade-authority {} \\\n  \
                    --payer {}",
                    program_id, additional_bytes, payer_keypair_path, payer_keypair_path
                )
            }
            spherenet_authority::Authority::MultiSig { vault, .. } => {
                format!(
                    "spherenet-admin program extend \\\n  \
                    --program-id {} \\\n  \
                    --bytes {} \\\n  \
                    --multisig {} \\\n  \
                    --multisig-authority <MEMBER_KEYPAIR> \\\n  \
                    --payer <MEMBER_KEYPAIR>",
                    program_id, additional_bytes, vault
                )
            }
        };

        return Err(eyre::eyre!(
            "❌ Program data account is too small!\n\n\
            Current capacity: {} bytes\n\
            Required capacity: {} bytes\n\
            Need {} more bytes\n\n\
            Run this command to extend the program:\n\
            {}",
            current_max_len,
            program_data.len(),
            additional_bytes,
            extend_cmd
        ));
    }

    println!("  ✓ Program capacity sufficient");

    // Verify upgrade authority is whitelisted before spending lamports (fail-fast)
    let whitelist_entry = require_whitelist_entry(&rpc_client, instruction_authority)?;

    // Create and write buffer
    println!("\n📝 Creating buffer account...");
    let buffer_keypair = Keypair::new();
    let buffer_pubkey = buffer_keypair.pubkey();
    let buffer_size = UpgradeableLoaderState::size_of_buffer(program_data.len());
    let buffer_lamports = rpc_client.get_minimum_balance_for_rent_exemption(buffer_size)?;

    println!("  Buffer size: {} bytes", buffer_size);
    println!("  Buffer rent: {} lamports", buffer_lamports);

    // Create and initialize buffer account with PAYER as buffer authority
    // (payer can write to buffer, then upgrade authority authorizes the upgrade)
    let create_buffer_instructions = create_buffer(
        &payer.pubkey(),
        &buffer_pubkey,
        &payer.pubkey(), // Payer is buffer authority for writes
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

    // Write program data to buffer in chunks (payer signs buffer writes)
    println!("\n📤 Writing program data to buffer...");
    write_buffer(
        &rpc_client,
        &payer,
        &payer, // Payer signs buffer writes
        &buffer_pubkey,
        &payer.pubkey(), // Buffer authority is payer
        &program_data,
    )?;

    // Transfer buffer authority to upgrade authority (required for multisig upgrades)
    println!("\n🔐 Transferring buffer authority...");
    let set_buffer_authority_ix = solana_sdk::bpf_loader_upgradeable::set_buffer_authority(
        &buffer_pubkey,
        &payer.pubkey(),        // Current buffer authority (payer)
        &instruction_authority, // New buffer authority (upgrade authority/vault PDA)
    );

    let mut set_authority_tx =
        Transaction::new_with_payer(&[set_buffer_authority_ix], Some(&payer.pubkey()));
    set_authority_tx.sign(&[&payer], rpc_client.get_latest_blockhash()?);
    rpc_client.send_and_confirm_transaction(&set_authority_tx)?;
    println!("  ✓ Buffer authority transferred to upgrade authority");

    // Upgrade program with whitelist validation
    println!("\n🎯 Upgrading program...");

    #[allow(deprecated)]
    let upgrade_ix = upgrade(
        &program_id,
        &buffer_pubkey,
        &instruction_authority, // Program's upgrade authority
        &spill_address,
        &whitelist_entry,
    );

    // Execute upgrade through authority (single-sig or multi-sig)
    let description = format!("Upgrade program {}", program_id);
    let result = upgrade_authority.execute_instruction(&rpc_client, upgrade_ix, &description)?;

    // Only show "upgraded successfully" for single-sig (immediate execution)
    if matches!(
        result,
        spherenet_authority::ExecutionResult::Executed { .. }
    ) {
        println!("\n✅ Program upgraded successfully!");
        println!("   Program ID: {}", program_id);
        println!("   Upgrade Authority: {}", instruction_authority);
    }

    Ok(())
}
