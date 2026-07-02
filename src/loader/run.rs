//! Program deployment operations: deploy, upgrade, and extend.

use crate::pw::run::require_whitelist_entry;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::bpf_loader_upgradeable;
use spherenet_authority::Authority;
#[allow(deprecated)]
use spherenet_whitelisted_loader_v3_interface::{
    instruction::{
        create_buffer, deploy_with_max_program_len, extend_program_checked, set_buffer_authority,
        upgrade, write,
    },
    state::UpgradeableLoaderState,
};
use std::{fs, str::FromStr};

/// Conservative chunk size for writing program data to avoid transaction size limits
const MAX_WRITE_SIZE: usize = 900;

/// Writes program data to a buffer account in chunks.
///
/// Program data is split into MAX_WRITE_SIZE chunks (900 bytes) to avoid hitting
/// transaction size limits. Each chunk is written via a separate transaction signed
/// by both the payer (who pays transaction fees) and the authority (buffer owner).
fn write_buffer(
    rpc_client: &RpcClient,
    payer: &Keypair,
    authority_keypair: &Keypair,
    buffer: &Pubkey,
    authority: &Pubkey,
    program_data: &[u8],
) -> eyre::Result<()> {
    let chunks: Vec<_> = program_data.chunks(MAX_WRITE_SIZE).collect();
    let total_chunks = chunks.len();

    for (i, chunk) in chunks.into_iter().enumerate() {
        let offset = i * MAX_WRITE_SIZE;
        print!(
            "  Writing chunk {}/{} ({} bytes)...",
            i + 1,
            total_chunks,
            chunk.len()
        );

        let write_ix = write(buffer, authority, offset as u32, chunk.to_vec());

        let mut transaction = Transaction::new_with_payer(&[write_ix], Some(&payer.pubkey()));
        transaction.sign(
            &[payer, authority_keypair],
            rpc_client.get_latest_blockhash()?,
        );
        rpc_client.send_and_confirm_transaction(&transaction)?;

        println!(" ✓");
    }

    println!("  ✓ Program data written successfully");
    Ok(())
}

/// Deploy a program to SphereNet
pub fn deploy(
    url: &str,
    program_so_path: String,
    program_keypair_path: String,
    upgrade_authority_path: String,
    payer_keypair_path: String,
    max_data_len: Option<usize>,
) -> eyre::Result<()> {
    println!("🚀 Deploying program to SphereNet...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Load keypairs
    let payer = read_keypair_file(&payer_keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read payer keypair from {}: {}",
            payer_keypair_path,
            e
        )
    })?;
    let program_keypair = read_keypair_file(&program_keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read program keypair from {}: {}",
            program_keypair_path,
            e
        )
    })?;
    let program_id = program_keypair.pubkey();
    let upgrade_authority_keypair = read_keypair_file(&upgrade_authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read upgrade authority keypair from {}: {}",
            upgrade_authority_path,
            e
        )
    })?;
    let upgrade_authority = upgrade_authority_keypair.pubkey();

    println!("  Program ID: {}", program_id);
    println!("  Payer: {}", payer.pubkey());
    println!("  Upgrade Authority: {}", upgrade_authority);

    // Read program .so file
    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre::eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    println!("  Program size: {} bytes", program_data.len());

    // Determine max data length
    let max_data_len_provided = max_data_len.is_some();
    let max_data_len = max_data_len.unwrap_or(program_data.len());
    if max_data_len < program_data.len() {
        return Err(eyre::eyre!(
            "max_data_len ({}) must be at least program size ({})",
            max_data_len,
            program_data.len()
        ));
    }

    // Warn if no max-data-len specified (important for multisig scenarios)
    if !max_data_len_provided {
        println!("\n⚠️  WARNING: No --max-data-len specified, using program size as capacity.");
        println!("   If you plan to transfer upgrade authority to multisig, you CANNOT extend later!");
        println!("   Consider deploying with generous --max-data-len (e.g., --max-data-len 500000)");
        println!();
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
    let (programdata_address, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id());

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
            spherenet_authority::Authority::MultiSig { multisig, .. } => {
                format!(
                    "spherenet-admin program extend \\\n  \
                    --program-id {} \\\n  \
                    --bytes {} \\\n  \
                    --multisig {} \\\n  \
                    --multisig-authority <MEMBER_KEYPAIR> \\\n  \
                    --payer <MEMBER_KEYPAIR>",
                    program_id, additional_bytes, multisig
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
    let set_buffer_authority_ix = set_buffer_authority(
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
    if matches!(result, spherenet_authority::ExecutionResult::Executed { .. }) {
        println!("\n✅ Program upgraded successfully!");
        println!("   Program ID: {}", program_id);
        println!("   Upgrade Authority: {}", instruction_authority);
    }

    Ok(())
}

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
    let extend_ix = extend_program_checked(
        &program_id,
        &instruction_authority,       // Authority required - vault PDA for multisig
        Some(&instruction_authority), // Payer also vault PDA for multisig consistency
        additional_bytes,
    );

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Extend program {} by {} bytes", program_id, additional_bytes);
    let result = upgrade_authority.execute_instruction(&rpc_client, extend_ix, &description)?;

    // Only show "extended successfully" for single-sig (immediate execution)
    if matches!(result, spherenet_authority::ExecutionResult::Executed { .. }) {
        println!("\n✅ Program data account extended successfully!");
    }

    Ok(())
}
