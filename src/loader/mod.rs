pub mod deploy;
pub mod upgrade;

use eyre::{eyre, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spherenet_program_whitelist_interface::{account_solana, program_solana};
use spherenet_whitelisted_loader_v3_interface::instruction::write;
use std::fs;

/// Conservative chunk size for writing program data to avoid transaction size limits
const MAX_WRITE_SIZE: usize = 900;

/// Verifies that the upgrade authority is whitelisted before spending any lamports.
///
/// This function performs a "fail-fast" check by:
/// 1. Deriving the whitelist PDA from the upgrade authority pubkey
/// 2. Querying the RPC to verify the whitelist entry account exists
///
/// Returns the whitelist PDA address if verification succeeds, allowing it to be
/// included in deploy/upgrade instructions (at account index 8 for deploy, 7 for upgrade).
///
/// # Arguments
/// * `rpc_client` - RPC client for querying account state
/// * `upgrade_authority` - Pubkey of the program's upgrade authority (must be whitelisted)
///
/// # Returns
/// * `Ok(Pubkey)` - The whitelist entry PDA if authority is whitelisted
/// * `Err` - If authority is not whitelisted, with instructions to add them
pub fn verify_whitelist_authority(
    rpc_client: &RpcClient,
    upgrade_authority: Pubkey,
) -> Result<Pubkey> {
    println!("\n🔐 Verifying whitelist...");
    let program_whitelist_program_id = Pubkey::new_from_array(program_solana::id().to_bytes());

    // Derive whitelist PDA: seeds = [program_whitelist_account_id, upgrade_authority]
    let (whitelist_entry, _bump) = Pubkey::find_program_address(
        &[
            &account_solana::id().to_bytes(),
            upgrade_authority.as_ref(),
        ],
        &program_whitelist_program_id,
    );

    println!("  Whitelist entry PDA: {}", whitelist_entry);

    // Verify whitelist entry account exists on-chain
    match rpc_client.get_account(&whitelist_entry) {
        Ok(_) => {
            println!("  ✓ Upgrade authority is whitelisted");
            Ok(whitelist_entry)
        }
        Err(_) => {
            Err(eyre!(
                "❌ Upgrade authority {} is not whitelisted!\n   Run: spherenet-admin pw add {} --auth <AUTHORITY>",
                upgrade_authority,
                upgrade_authority
            ))
        }
    }
}

/// Writes program data to a buffer account in chunks.
///
/// Program data is split into MAX_WRITE_SIZE chunks (900 bytes) to avoid hitting
/// transaction size limits. Each chunk is written via a separate transaction signed
/// by both the payer (who pays transaction fees) and the authority (buffer owner).
///
/// # Arguments
/// * `rpc_client` - RPC client for submitting transactions
/// * `payer` - Keypair that pays for transaction fees
/// * `authority_keypair` - Buffer authority keypair (must sign each write)
/// * `buffer` - Address of the buffer account to write to
/// * `authority` - Authority pubkey (must match buffer's authority)
/// * `program_data` - Complete program bytecode to write
pub fn write_buffer(
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

/// Loads a Solana keypair from a file, supporting both JSON and binary formats.
///
/// Detects format by checking first byte:
/// - `[` = JSON array format (e.g., `[1, 2, 3, ...]`)
/// - Otherwise = binary format (raw 64 bytes)
///
/// # Arguments
/// * `path` - Absolute or relative path to the keypair file
///
/// # Returns
/// * `Ok(Keypair)` - Successfully loaded keypair
/// * `Err` - If file cannot be read or parsed
pub fn load_keypair(path: &str) -> Result<Keypair> {
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
