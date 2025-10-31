use crate::consts::SYSTEM_PROGRAM;
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_program_whitelist_client::instructions::{AddEntryBuilder, RemoveEntryBuilder};
use spherenet_program_whitelist_interface::{
    account_solana, program_solana,
    state::{load, whitelist_entry::ProgramWhitelistEntry},
};

pub fn list(rpc_url: &str) -> Result<()> {
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

pub fn add(rpc_url: &str, program_authority: String, authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse deployer authority (who can deploy/upgrade programs)
    let deployer_pubkey = program_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deployer authority: {}", e))?;

    // Load whitelist authority keypair
    let authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load whitelist authority keypair from {}: {}",
            authority_path,
            e
        )
    })?;

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let program_id = Pubkey::from(program_solana::id().to_bytes());

    // Derive the whitelist entry PDA using solana_sdk (not pinocchio)
    let (whitelist_entry_pda, _bump) = Pubkey::find_program_address(
        &[whitelist_pubkey.as_ref(), deployer_pubkey.as_ref()],
        &program_id,
    );

    println!("\nWhitelisting deployer authority:");
    println!("  Deployer Authority:  {}", deployer_pubkey);
    println!("  Whitelist Entry PDA: {}", whitelist_entry_pda);
    println!("  Whitelist Authority: {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = AddEntryBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(authority_keypair.pubkey())
        .whitelist_entry_account(whitelist_entry_pda)
        .payer(authority_keypair.pubkey())
        .system_program(*SYSTEM_PROGRAM)
        .program_authority(deployer_pubkey)
        .instruction();

    // Get recent blockhash and create transaction
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&authority_keypair.pubkey()),
        &[&authority_keypair],
        recent_blockhash,
    );

    // Send transaction
    println!("\nSending transaction...");
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("  Signature: {}", signature);
    println!("\nDeployer authority whitelisted successfully!");
    println!("This authority can now deploy and upgrade programs on the network.");

    Ok(())
}

pub fn remove(rpc_url: &str, program_authority: String, authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse deployer authority (who can deploy/upgrade programs)
    let deployer_pubkey = program_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deployer authority: {}", e))?;

    // Load whitelist authority keypair
    let authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load whitelist authority keypair from {}: {}",
            authority_path,
            e
        )
    })?;

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let program_id = Pubkey::from(program_solana::id().to_bytes());

    // Derive the whitelist entry PDA using solana_sdk (not pinocchio)
    let (whitelist_entry_pda, _bump) = Pubkey::find_program_address(
        &[whitelist_pubkey.as_ref(), deployer_pubkey.as_ref()],
        &program_id,
    );

    println!("\nRemoving deployer authority from whitelist:");
    println!("  Deployer Authority:  {}", deployer_pubkey);
    println!("  Whitelist Entry PDA: {}", whitelist_entry_pda);
    println!("  Whitelist Authority: {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = RemoveEntryBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(authority_keypair.pubkey())
        .whitelist_entry_account(whitelist_entry_pda)
        .destination_account(authority_keypair.pubkey()) // Reclaim lamports to authority
        .system_program(*SYSTEM_PROGRAM)
        .program_authority(deployer_pubkey)
        .instruction();

    // Get recent blockhash and create transaction
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&authority_keypair.pubkey()),
        &[&authority_keypair],
        recent_blockhash,
    );

    // Send transaction
    println!("\nSending transaction...");
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("  Signature: {}", signature);
    println!("\nDeployer authority removed from whitelist successfully!");
    println!("This authority can no longer deploy or upgrade programs on the network.");

    Ok(())
}
