use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_authority::Authority;
use spherenet_program_whitelist_client::instructions::{AddEntryBuilder, RemoveEntryBuilder};
use spherenet_program_whitelist_interface::{
    account_solana, program_solana,
    state::{load, whitelist_entry::ProgramWhitelistEntry},
};
use std::sync::LazyLock;

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(Pubkey::default);

/// Derives the program whitelist entry PDA for a deployer authority.
///
/// The PDA is derived using:
/// - Seeds: `[program_whitelist_account_id, deployer_authority]`
/// - Program: program whitelist program ID
///
/// # Arguments
/// * `deployer_authority` - Pubkey of the authority who can deploy/upgrade programs
///
/// # Returns
/// * `(Pubkey, u8)` - The derived PDA and bump seed
pub fn derive_whitelist_entry(deployer_authority: &Pubkey) -> (Pubkey, u8) {
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    Pubkey::find_program_address(
        &[whitelist_pubkey.as_ref(), deployer_authority.as_ref()],
        &program_id,
    )
}

pub fn list(rpc_url: &str) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    // Use getProgramAccounts to find all program whitelist entries
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let accounts = rpc_client.get_program_accounts(&program_id)?;

    // Count entries first
    let mut entries = Vec::new();
    for (pubkey, account) in accounts {
        // Skip the main whitelist account itself
        if pubkey == whitelist_pubkey {
            continue;
        }

        // Try to deserialize as ProgramWhitelistEntry
        if let Ok(entry_data) = load::<ProgramWhitelistEntry>(&account.data) {
            entries.push(Pubkey::from(entry_data.entry_address));
        }
    }

    if entries.is_empty() {
        println!("\nNo deployer authorities whitelisted.");
        return Ok(());
    }

    println!("\nWhitelisted Deployer Authorities ({}):", entries.len());

    for deployer in entries {
        println!("  Deployer Authority: {}", deployer);
    }
    println!();

    Ok(())
}

pub fn add(rpc_url: &str, program_authority: String, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse deployer authority (who can deploy/upgrade programs)
    let deployer_pubkey = program_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deployer authority: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deployer_pubkey);

    println!("\nWhitelisting deployer authority:");
    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("  Deployer Authority:  {}", deployer_pubkey);
    println!("  Whitelist Entry PDA: {}", whitelist_entry_pda);
    println!("  Whitelist Authority: {}", instruction_authority);

    // Build the instruction
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction = AddEntryBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(instruction_authority)
        .whitelist_entry_account(whitelist_entry_pda)
        .payer(instruction_authority)
        .system_program(*SYSTEM_PROGRAM)
        .program_authority(deployer_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Whitelist deployer authority {}", deployer_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn remove(rpc_url: &str, program_authority: String, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse deployer authority (who can deploy/upgrade programs)
    let deployer_pubkey = program_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deployer authority: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deployer_pubkey);

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nRemoving deployer authority from whitelist:");
    println!("  Deployer Authority:  {}", deployer_pubkey);
    println!("  Whitelist Entry PDA: {}", whitelist_entry_pda);
    println!("  Whitelist Authority: {}", instruction_authority);

    // Build the instruction
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction = RemoveEntryBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(instruction_authority)
        .whitelist_entry_account(whitelist_entry_pda)
        .destination_account(instruction_authority) // Reclaim lamports to authority
        .system_program(*SYSTEM_PROGRAM)
        .program_authority(deployer_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Remove deployer authority {} from whitelist",
        deployer_pubkey
    );
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

/// Requires that the upgrade authority is whitelisted, returning the whitelist entry PDA.
///
/// This function performs a "fail-fast" check before spending any lamports by:
/// 1. Deriving the whitelist PDA from the upgrade authority pubkey
/// 2. Querying the RPC to verify the whitelist entry account exists
/// 3. Returning the PDA for use in deploy/upgrade instructions (account index 8 for deploy, 7 for upgrade)
///
/// # Arguments
/// * `rpc_client` - RPC client for querying account state
/// * `upgrade_authority` - Pubkey of the program's upgrade authority (must be whitelisted)
///
/// # Returns
/// * `Ok(Pubkey)` - The whitelist entry PDA if authority is whitelisted
/// * `Err` - If authority is not whitelisted, with instructions to add them
pub fn require_whitelist_entry(
    rpc_client: &RpcClient,
    upgrade_authority: Pubkey,
) -> eyre::Result<Pubkey> {
    println!("\n🔐 Verifying whitelist...");

    // Derive whitelist PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&upgrade_authority);

    println!("  Whitelist entry PDA: {}", whitelist_entry_pda);

    // Verify whitelist entry account exists on-chain
    match rpc_client.get_account(&whitelist_entry_pda) {
        Ok(_) => {
            println!("  ✓ Upgrade authority is whitelisted");
            Ok(whitelist_entry_pda)
        }
        Err(_) => {
            Err(eyre::eyre!(
                "❌ Upgrade authority {} is not whitelisted!\n   Run: spherenet-admin pw add {} --auth <AUTHORITY>",
                upgrade_authority,
                upgrade_authority
            ))
        }
    }
}
