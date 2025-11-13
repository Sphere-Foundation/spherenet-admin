//! Multisig command implementations
//!
//! User-facing CLI commands for managing multisig vaults and proposals.
//! Handles parsing, validation, user feedback, and formatting.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::AccountMeta, pubkey::Pubkey, signature::Signer, transaction::Transaction,
};
use std::str::FromStr;

use borsh::BorshDeserialize;

use crate::squads::{self, Member, Multisig, Permissions, ProgramConfigInitArgs};

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
) -> eyre::Result<()> {
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
) -> eyre::Result<()> {
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
        .collect::<eyre::Result<Vec<_>>>()?;

    if member_pubkeys.is_empty() {
        eyre::bail!("At least one member is required");
    }

    // Validate threshold
    if threshold == 0 {
        eyre::bail!("Threshold must be at least 1");
    }
    if threshold as usize > member_pubkeys.len() {
        eyre::bail!(
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
        eyre::bail!("Invalid program_config account data");
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

/// Get vault address from create key
///
/// # Arguments
/// * `create_key_path` - Path to the create key keypair used during vault creation
pub fn vault_address(create_key_path: String) -> eyre::Result<()> {
    // Load create key
    let create_key = solana_sdk::signature::read_keypair_file(&create_key_path)
        .map_err(|e| eyre::eyre!("Failed to read create key '{}': {}", create_key_path, e))?;

    // Parse program ID
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // Derive multisig PDA
    let (multisig_pda, bump) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);

    println!("\nVault Address:");
    println!("  Create Key: {}", create_key.pubkey());
    println!("  Vault:      {}", multisig_pda);
    println!("  Bump:       {}", bump);
    println!();

    Ok(())
}

/// Show multisig information (fetches on-chain data)
///
/// # Arguments
/// * `create_key_path` - Path to the create key keypair used during vault creation
/// * `url` - RPC URL
pub fn show_multisig(create_key_path: String, url: &str) -> eyre::Result<()> {
    // 1. Load create key
    let create_key = solana_sdk::signature::read_keypair_file(&create_key_path)
        .map_err(|e| eyre::eyre!("Failed to read create key '{}': {}", create_key_path, e))?;

    // 2. Derive multisig PDA
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
    let (multisig_pda, _bump) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);

    // 3. Fetch account
    let rpc = RpcClient::new(url);
    let account = rpc.get_account(&multisig_pda).map_err(|e| {
        eyre::eyre!(
            "Failed to fetch multisig account at {}: {}",
            multisig_pda,
            e
        )
    })?;

    // 4. Deserialize account data (skip 8-byte Anchor discriminator)
    if account.data.len() < 8 {
        eyre::bail!("Invalid multisig account data (too short)");
    }
    // Use deserialize_reader to handle accounts with extra allocated space
    let mut data_slice = &account.data[8..];
    let multisig_data = Multisig::deserialize_reader(&mut data_slice)
        .map_err(|e| eyre::eyre!("Failed to deserialize multisig account: {}", e))?;

    // 5. Get SOL balance
    let balance_lamports = rpc.get_balance(&multisig_pda)?;
    let balance_sol = balance_lamports as f64 / 1_000_000_000.0;

    // 6. Display information
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║               Multisig Vault Information                      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("Create Key:     {}", multisig_data.create_key);
    println!("Vault (PDA):    {}", multisig_pda);
    println!();
    println!(
        "Threshold:      {}/{}",
        multisig_data.threshold,
        multisig_data.members.len()
    );
    println!("Next TX Index:  {}", multisig_data.transaction_index);
    println!("Stale TX Index: {}", multisig_data.stale_transaction_index);
    println!("Time Lock:      {} seconds", multisig_data.time_lock);
    println!(
        "SOL Balance:    {:.9} SOL ({} lamports)",
        balance_sol, balance_lamports
    );
    println!();

    println!("Config Authority:");
    if multisig_data.config_authority == Pubkey::default() {
        println!("  Autonomous (no external authority)");
    } else {
        println!("  {}", multisig_data.config_authority);
    }
    println!();

    println!("Rent Collector:");
    match multisig_data.rent_collector {
        Some(rc) => println!("  {}", rc),
        None => println!("  Disabled"),
    }
    println!();

    println!("Members ({}):", multisig_data.members.len());
    for (i, member) in multisig_data.members.iter().enumerate() {
        let perms = format_permissions(&member.permissions);
        println!("  [{}] {} ({})", i + 1, member.key, perms);
    }
    println!();

    Ok(())
}

/// Helper to format permissions as readable string
fn format_permissions(perms: &Permissions) -> String {
    let mut parts = Vec::new();
    if perms.mask & (squads::types::Permission::Initiate as u8) != 0 {
        parts.push("Initiate");
    }
    if perms.mask & (squads::types::Permission::Vote as u8) != 0 {
        parts.push("Vote");
    }
    if perms.mask & (squads::types::Permission::Execute as u8) != 0 {
        parts.push("Execute");
    }
    if parts.is_empty() {
        "No permissions".to_string()
    } else {
        parts.join(" | ")
    }
}

/// Approve a multisig proposal
///
/// # Arguments
/// * `create_key_path` - Path to the create key used during vault creation
/// * `transaction_index` - Transaction index of the proposal to approve
/// * `member_path` - Path to the member keypair who is approving
/// * `url` - RPC URL
pub fn approve_proposal(
    create_key_path: String,
    transaction_index: u64,
    member_path: String,
    url: &str,
) -> eyre::Result<()> {
    println!("\nApproving multisig proposal...");
    println!("  Transaction Index: {}", transaction_index);
    println!();

    // Load keys
    let create_key = solana_sdk::signature::read_keypair_file(&create_key_path)
        .map_err(|e| eyre::eyre!("Failed to read create key: {}", e))?;

    let member = solana_sdk::signature::read_keypair_file(&member_path)
        .map_err(|e| eyre::eyre!("Failed to read member key: {}", e))?;

    // Parse program ID and derive PDAs
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
    let (multisig_pda, _) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);
    let (proposal_pda, _) =
        squads::types::get_proposal_pda(&multisig_pda, transaction_index - 1, &program_id);

    println!("  Vault:    {}", multisig_pda);
    println!("  Proposal: {}", proposal_pda);
    println!("  Member:   {}", member.pubkey());
    println!();

    // Build approve instruction
    let approve_ix = squads::instructions::build_proposal_approve_ix(
        &program_id,
        &multisig_pda,
        &proposal_pda,
        &member.pubkey(),
    )?;

    // Send transaction
    let rpc = RpcClient::new(url);
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[approve_ix],
        Some(&member.pubkey()),
        &[&member],
        recent_blockhash,
    );

    println!("Sending approval transaction...");
    let signature = rpc.send_and_confirm_transaction(&tx)?;

    println!();
    println!("✅ Proposal approved!");
    println!("   Signature: {}", signature);
    println!();

    Ok(())
}

/// Execute an approved multisig proposal
///
/// # Arguments
/// * `create_key_path` - Path to the create key used during vault creation  
/// * `transaction_index` - Transaction index of the proposal to execute
/// * `member_path` - Path to the member keypair who is executing
/// * `url` - RPC URL
pub fn execute_proposal(
    create_key_path: String,
    transaction_index: u64,
    member_path: String,
    url: &str,
) -> eyre::Result<()> {
    println!(
        "
Executing multisig proposal..."
    );
    println!("  Transaction Index: {}", transaction_index);
    println!();

    // Load keys
    let create_key = solana_sdk::signature::read_keypair_file(&create_key_path)
        .map_err(|e| eyre::eyre!("Failed to read create key: {}", e))?;

    let member = solana_sdk::signature::read_keypair_file(&member_path)
        .map_err(|e| eyre::eyre!("Failed to read member key: {}", e))?;

    // Parse program ID and derive PDAs
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
    let (multisig_pda, _) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);
    let (proposal_pda, _) =
        squads::types::get_proposal_pda(&multisig_pda, transaction_index - 1, &program_id);
    let (vault_transaction_pda, _) =
        squads::types::get_vault_transaction_pda(&multisig_pda, transaction_index - 1, &program_id);

    println!("  Vault:        {}", multisig_pda);
    println!("  Proposal:     {}", proposal_pda);
    println!("  Transaction:  {}", vault_transaction_pda);
    println!("  Member:       {}", member.pubkey());
    println!();
    println!("  Fetching VaultTransaction from: {}", vault_transaction_pda);
    println!();

    // Fetch and parse the vault transaction to get the exact accounts
    let rpc = RpcClient::new(url);
    let vault_tx_account = rpc
        .get_account(&vault_transaction_pda)
        .map_err(|e| eyre::eyre!("Failed to fetch vault transaction: {}", e))?;

    println!("  Fetched account data length: {} bytes", vault_tx_account.data.len());
    if vault_tx_account.data.len() >= 80 {
        let creator_bytes: [u8; 32] = vault_tx_account.data[40..72].try_into().unwrap();
        let creator_pubkey = Pubkey::from(creator_bytes);
        println!("  Creator pubkey (bytes 40-72): {}", creator_pubkey);
    }

    // Deserialize VaultTransaction (skip 8-byte Anchor discriminator)
    if vault_tx_account.data.len() < 8 {
        eyre::bail!("Invalid vault transaction account data");
    }
    let data_slice = &vault_tx_account.data[8..];
    let vault_tx = squads::types::VaultTransaction::deserialize(&mut &data_slice[..])
        .map_err(|e| eyre::eyre!("Failed to deserialize vault transaction: {}", e))?;

    // Derive vault PDA using find_program_address (canonical bump)
    let (our_vault_pda, our_vault_bump) = squads::types::get_vault_pda(&multisig_pda, vault_tx.vault_index, &program_id);

    // Derive vault PDA using stored bump (what the program uses)
    let stored_vault_pda = Pubkey::create_program_address(
        &[
            squads::types::SEED_PREFIX,
            multisig_pda.as_ref(),
            squads::types::SEED_VAULT,
            &vault_tx.vault_index.to_le_bytes(),
            &[vault_tx.vault_bump],
        ],
        &program_id,
    ).map_err(|e| eyre::eyre!("Failed to derive vault PDA with stored bump: {}", e))?;

    println!("  Vault PDA derivation:");
    println!("    Vault index:       {}", vault_tx.vault_index);
    println!("    Our derived PDA:   {} (bump: {})", our_vault_pda, our_vault_bump);
    println!("    Stored bump:       {}", vault_tx.vault_bump);
    println!("    Stored PDA:        {}", stored_vault_pda);
    println!("    PDAs match:        {}", our_vault_pda == stored_vault_pda);
    println!();

    println!("  Transaction message details:");
    println!("    num_signers: {}", vault_tx.message.num_signers);
    println!("    num_writable_signers: {}", vault_tx.message.num_writable_signers);
    println!("    num_writable_non_signers: {}", vault_tx.message.num_writable_non_signers);
    println!("  Account keys in transaction message:");
    for (i, key) in vault_tx.message.account_keys.iter().enumerate() {
        println!("    [{}] {}", i, key);
    }
    println!();

    // Build remaining_accounts from the message's account_keys
    // We need to determine which are writable/readonly based on the message headers
    let num_writable_signers = vault_tx.message.num_writable_signers as usize;
    let num_readonly_signers =
        (vault_tx.message.num_signers - vault_tx.message.num_writable_signers) as usize;
    let num_writable_non_signers = vault_tx.message.num_writable_non_signers as usize;

    // Determine if each account is a signer based on the message headers
    // PDAs (vault and ephemeral signers) cannot be marked as signers
    let is_signer_index = |index: usize| -> bool {
        index < vault_tx.message.num_signers as usize
    };

    let mut remaining_accounts = Vec::new();
    for (i, key) in vault_tx.message.account_keys.iter().enumerate() {
        let is_writable = if i < num_writable_signers {
            true // Writable signer
        } else if i < num_writable_signers + num_readonly_signers {
            false // Readonly signer
        } else if i < num_writable_signers + num_readonly_signers + num_writable_non_signers {
            true // Writable non-signer
        } else {
            false // Readonly non-signer
        };

        // From Squads SDK: PDAs (vault_pda and ephemeral signers) cannot be marked as signers
        let is_signer = is_signer_index(i) && key != &stored_vault_pda;

        remaining_accounts.push(AccountMeta {
            pubkey: *key,
            is_signer,
            is_writable,
        });
    }

    println!("  Remaining accounts we're providing:");
    for (i, acc) in remaining_accounts.iter().enumerate() {
        println!(
            "    [{}] {} (writable: {}, signer: {})",
            i, acc.pubkey, acc.is_writable, acc.is_signer
        );
    }
    println!();

    // Build execute instruction
    let execute_ix = squads::instructions::build_vault_transaction_execute_ix(
        &program_id,
        &multisig_pda,
        &proposal_pda,
        &vault_transaction_pda,
        &member.pubkey(),
        remaining_accounts,
    )?;

    // Send transaction
    let rpc = RpcClient::new(url);
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[execute_ix],
        Some(&member.pubkey()),
        &[&member],
        recent_blockhash,
    );

    println!("Sending execution transaction...");
    let signature = rpc.send_and_confirm_transaction(&tx)?;

    println!();
    println!("✅ Proposal executed!");
    println!("   Signature: {}", signature);
    println!();

    Ok(())
}
