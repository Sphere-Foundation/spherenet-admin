//! Multisig command implementations
//!
//! User-facing CLI commands for managing multisig vaults and proposals.
//! Handles parsing, validation, user feedback, and formatting.

use eyre::{bail, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Signer, transaction::Transaction};
use std::str::FromStr;

use crate::squads::{self, Member, Permissions, ProgramConfigInitArgs};

/// Initialize the Squads program config (one-time setup)
///
/// # Arguments
/// * `authority` - Pubkey that will control the program config
/// * `treasury` - Pubkey where multisig creation fees are sent
/// * `creation_fee` - Fee in lamports charged for creating a multisig
/// * `initializer_path` - Path to the INITIALIZER keypair (hardcoded in program)
/// * `url` - RPC URL
pub fn program_config_init(
    authority: String,
    treasury: String,
    creation_fee: u64,
    initializer_path: String,
    url: &str,
) -> Result<()> {
    println!("Initializing Squads program config...");
    println!();

    // Parse pubkeys
    let authority_pubkey = Pubkey::from_str(&authority)
        .map_err(|e| eyre::eyre!("Invalid authority pubkey '{}': {}", authority, e))?;

    let treasury_pubkey = Pubkey::from_str(&treasury)
        .map_err(|e| eyre::eyre!("Invalid treasury pubkey '{}': {}", treasury, e))?;

    // Show configuration
    println!("Configuration:");
    println!("  Authority: {}", authority_pubkey);
    println!("  Treasury:  {}", treasury_pubkey);
    println!(
        "  Creation Fee: {} lamports ({:.6} SOL)",
        creation_fee,
        creation_fee as f64 / 1_000_000_000.0
    );
    println!();

    // Load initializer keypair
    let initializer = solana_sdk::signature::read_keypair_file(&initializer_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read initializer key '{}': {}",
            initializer_path,
            e
        )
    })?;

    println!("Initializer: {}", initializer.pubkey());
    println!();

    // Create RPC client
    let rpc = RpcClient::new(url);

    // Parse program ID
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // Derive program config PDA
    let (program_config_pda, _) = squads::types::get_program_config_pda(&program_id);

    // Build args
    let args = ProgramConfigInitArgs {
        authority: authority_pubkey,
        multisig_creation_fee: creation_fee,
        treasury: treasury_pubkey,
    };

    // Build instruction
    let init_ix = squads::instructions::build_program_config_init_ix(
        &program_id,
        &program_config_pda,
        &initializer.pubkey(),
        args,
    )?;

    println!("Sending transaction...");

    // Send transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;

    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&initializer.pubkey()),
        &[&initializer],
        recent_blockhash,
    );

    let signature = rpc.send_and_confirm_transaction(&tx)?;

    println!();
    println!("✅ Program config initialized successfully!");
    println!();
    println!("   Program Config PDA: {}", program_config_pda);
    println!("   Transaction: {}", signature);
    println!();
    println!("The Squads program is now ready to create multisig vaults.");
    println!();

    Ok(())
}

/// Create a new multisig vault
///
/// # Arguments
/// * `members_str` - Comma-separated list of member pubkeys
/// * `threshold` - Number of approvals required
/// * `create_key_path` - Path to keypair for PDA derivation
/// * `payer_path` - Path to payer keypair
/// * `url` - RPC URL
/// * `time_lock` - Optional time delay (seconds)
/// * `memo` - Optional description
pub fn create(
    members_str: String,
    threshold: u16,
    create_key_path: String,
    payer_path: String,
    url: &str,
    time_lock: Option<u32>,
    memo: Option<String>,
) -> Result<()> {
    println!("Creating Squads v4 multisig vault...");
    println!();

    // Parse member pubkeys
    let member_pubkeys: Vec<Pubkey> = members_str
        .split(',')
        .map(|s| {
            let trimmed = s.trim();
            Pubkey::from_str(trimmed)
                .map_err(|e| eyre::eyre!("Invalid pubkey '{}': {}", trimmed, e))
        })
        .collect::<Result<Vec<_>>>()?;

    if member_pubkeys.is_empty() {
        bail!("At least one member is required");
    }

    // Validate threshold
    if threshold == 0 {
        bail!("Threshold must be at least 1");
    }
    if threshold as usize > member_pubkeys.len() {
        bail!(
            "Threshold ({}) cannot exceed number of members ({})",
            threshold,
            member_pubkeys.len()
        );
    }

    // Show what we're creating
    println!("Configuration:");
    println!("  Members: {}", member_pubkeys.len());
    for (i, pk) in member_pubkeys.iter().enumerate() {
        println!("    [{}] {}", i + 1, pk);
    }
    println!("  Threshold: {}/{}", threshold, member_pubkeys.len());
    if let Some(tl) = time_lock {
        if tl > 0 {
            println!("  Time Lock: {} seconds", tl);
        }
    }
    if let Some(ref m) = memo {
        println!("  Memo: {}", m);
    }
    println!();

    // Load keypairs
    let create_key = solana_sdk::signature::read_keypair_file(&create_key_path)
        .map_err(|e| eyre::eyre!("Failed to read create key '{}': {}", create_key_path, e))?;

    let payer = solana_sdk::signature::read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer key '{}': {}", payer_path, e))?;

    // Create RPC client
    let rpc = RpcClient::new(url);

    // Parse program ID
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // Derive PDAs
    let (multisig_pda, _) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);
    let (program_config_pda, _) = squads::types::get_program_config_pda(&program_id);

    // Fetch program config to get treasury
    let program_config_account = rpc.get_account(&program_config_pda)
        .map_err(|e| eyre::eyre!("Failed to fetch program_config account at {}: {}. Has program_config_init been called?", program_config_pda, e))?;

    // Parse treasury from account data
    // ProgramConfig layout: discriminator(8) + authority(32) + multisig_creation_fee(8) + treasury(32)
    if program_config_account.data.len() < 80 {
        bail!("Invalid program_config account data");
    }
    let treasury_bytes: [u8; 32] = program_config_account.data[48..80].try_into()?;
    let treasury = Pubkey::new_from_array(treasury_bytes);

    // Build members with full permissions
    let members: Vec<Member> = member_pubkeys
        .iter()
        .map(|key| Member {
            key: *key,
            permissions: Permissions::all(),
        })
        .collect();

    // Build args
    let args = squads::types::MultisigCreateArgsV2 {
        config_authority: None, // Immutable multisig
        threshold,
        members,
        time_lock: time_lock.unwrap_or(0),
        rent_collector: None, // Defaults to creator
        memo,
    };

    // Build instruction
    let create_ix = squads::instructions::build_multisig_create_v2_ix(
        &program_id,
        &program_config_pda,
        &treasury, // Must match program_config.treasury
        &multisig_pda,
        &create_key.pubkey(),
        &payer.pubkey(),
        args,
    )?;

    println!("Sending transaction...");

    // Send transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;

    let tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&payer.pubkey()),
        &[&payer, &create_key], // Both must sign
        recent_blockhash,
    );

    let _signature = rpc.send_and_confirm_transaction(&tx)?;

    println!();
    println!("✅ Multisig vault created successfully!");
    println!();
    println!("   Vault Address: {}", multisig_pda);
    println!();
    println!("Use this address with --multisig-authority in spherenet-admin commands:");
    println!(
        "  spherenet-admin pw add <DEPLOYER> --multisig-authority {} --signer <MEMBER_KEYPAIR>",
        multisig_pda
    );
    println!();

    Ok(())
}
