use eyre::{eyre, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spherenet_program_whitelist_interface::{account_solana, program_solana};
#[allow(deprecated)]
use spherenet_whitelisted_loader_v3_interface::{
    instruction::{create_buffer, deploy_with_max_program_len, write},
    state::UpgradeableLoaderState,
};
use std::fs;

const MAX_WRITE_SIZE: usize = 900; // Conservative chunk size for writing program data

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
    let payer = load_keypair(&payer_keypair_path)?;
    let program_keypair = load_keypair(&program_keypair_path)?;
    let program_id = program_keypair.pubkey();
    let upgrade_authority_keypair = load_keypair(&upgrade_authority_str)?;
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

    // Step 1: Derive and verify whitelist PDA FIRST (before spending lamports)
    println!("\n🔐 Verifying whitelist...");
    let program_whitelist_program_id = Pubkey::new_from_array(program_solana::id().to_bytes());

    let (whitelist_entry, _bump) = Pubkey::find_program_address(
        &[
            &account_solana::id().to_bytes(), // Convert __Pubkey to [u8; 32]
            upgrade_authority.as_ref(),
        ],
        &program_whitelist_program_id,
    );

    println!("  Whitelist entry PDA: {}", whitelist_entry);

    // Verify whitelist entry exists
    match rpc_client.get_account(&whitelist_entry) {
        Ok(_) => println!("  ✓ Upgrade authority is whitelisted"),
        Err(_) => {
            return Err(eyre!(
                "❌ Upgrade authority {} is not whitelisted!\n   Run: spherenet-admin pw add {} --auth <AUTHORITY>",
                upgrade_authority,
                upgrade_authority
            ));
        }
    }

    // Step 2: Create and write buffer
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

    // Step 3: Write program data to buffer (upgrade authority signs as buffer authority)
    println!("\n📤 Writing program data to buffer...");
    write_buffer(
        &rpc_client,
        &payer,
        &upgrade_authority_keypair,
        &buffer_pubkey,
        &upgrade_authority,
        &program_data,
    )?;

    // Step 4: Deploy program
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

/// Write program data to buffer in chunks
fn write_buffer(
    rpc_client: &RpcClient,
    payer: &Keypair,
    authority_keypair: &Keypair,
    buffer: &Pubkey,
    authority: &Pubkey,
    program_data: &[u8],
) -> Result<()> {
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

/// Load a keypair from a file path
fn load_keypair(path: &str) -> Result<Keypair> {
    let keypair_bytes =
        fs::read(path).map_err(|e| eyre!("Failed to read keypair file {}: {}", path, e))?;

    let keypair = if keypair_bytes[0] == b'[' {
        // JSON format
        let keypair_vec: Vec<u8> = serde_json::from_slice(&keypair_bytes)
            .map_err(|e| eyre!("Failed to parse keypair JSON: {}", e))?;
        Keypair::try_from(keypair_vec.as_slice())
            .map_err(|e| eyre!("Failed to create keypair from bytes: {}", e))?
    } else {
        // Binary format
        Keypair::try_from(keypair_bytes.as_slice())
            .map_err(|e| eyre!("Failed to create keypair from bytes: {}", e))?
    };

    Ok(keypair)
}
