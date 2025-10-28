use crate::consts::{RPC_URL, SYSTEM_PROGRAM};
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_validator_whitelist_client::instructions::{
    AcceptAuthorityTransferBuilder, AddToWhitelistBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder, RemoveFromWhitelistBuilder, UpdateEndEpochBuilder,
    UpdateStartEpochBuilder,
};
use spherenet_validator_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ValidatorWhitelistAccount, load, whitelist_entry::ValidatorWhitelistEntry},
};

pub fn list() -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ValidatorWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize whitelist account: {:?}", e))?;

    // Get validator count
    let validator_count = u32::from_le_bytes(whitelist.validator_amount);

    if validator_count == 0 {
        println!("\nNo validators whitelisted.");
        return Ok(());
    }

    println!("\nWhitelisted Validators ({}):", validator_count);

    // Use getProgramAccounts to find all validator whitelist entries
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let accounts = rpc_client.get_program_accounts(&program_id)?;

    let mut found_count = 0;
    for (pubkey, account) in accounts {
        // Skip the main whitelist account itself
        if pubkey == whitelist_pubkey {
            continue;
        }

        // Try to deserialize as ValidatorWhitelistEntry
        if let Ok(entry_data) = load::<ValidatorWhitelistEntry>(&account.data) {
            let start_epoch = u64::from_le_bytes(entry_data.start_epoch);
            let end_epoch = u64::from_le_bytes(entry_data.end_epoch);

            println!("  Vote Account:  {}", Pubkey::from(entry_data.pubkey));
            println!("  Start Epoch:   {}", start_epoch);
            println!(
                "  End Epoch:     {}",
                if end_epoch == u64::MAX {
                    "∞".to_string()
                } else {
                    end_epoch.to_string()
                }
            );
            println!();

            found_count += 1;
        }
    }

    if found_count == 0 {
        println!("  No validator entries found");
    }

    Ok(())
}

pub fn auth() -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ValidatorWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize whitelist account: {:?}", e))?;

    // Print authority info
    println!("\nValidator Whitelist Authority");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", Pubkey::from(whitelist.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(whitelist.pending_authority)
    );
    println!(
        "  Validator Count:   {}",
        u32::from_le_bytes(whitelist.validator_amount)
    );
    println!();

    Ok(())
}

pub fn add(
    vote_account: String,
    start_epoch: Option<u64>,
    end_epoch: Option<u64>,
    keypair_path: String,
) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Load authority keypair
    let authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Get current epoch if start_epoch not provided
    let current_epoch = rpc_client.get_epoch_info()?.epoch;
    let start_epoch = start_epoch.unwrap_or(current_epoch + 1);
    let end_epoch = end_epoch.unwrap_or(u64::MAX);

    // Validate epochs
    if start_epoch >= end_epoch {
        return Err(eyre::eyre!("Start epoch must be less than end epoch"));
    }

    if start_epoch < current_epoch {
        return Err(eyre::eyre!(
            "Start epoch {} is in the past (current epoch: {})",
            start_epoch,
            current_epoch
        ));
    }

    // Derive the whitelist entry PDA
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let (whitelist_entry_pda, _bump) =
        Pubkey::find_program_address(&[vote_account_pubkey.as_ref()], &program_id);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nAdding validator to whitelist:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!("  Start Epoch:     {}", start_epoch);
    println!(
        "  End Epoch:       {}",
        if end_epoch == u64::MAX {
            "∞".to_string()
        } else {
            end_epoch.to_string()
        }
    );
    println!("  Authority:       {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = AddToWhitelistBuilder::new()
        .payer(authority_keypair.pubkey())
        .whitelist_authority(authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .validator_vote_account(vote_account_pubkey)
        .system_program(*SYSTEM_PROGRAM)
        .start_epoch(start_epoch.to_le_bytes())
        .end_epoch(end_epoch.to_le_bytes())
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
    println!("\nValidator added successfully!");

    Ok(())
}

pub fn remove(vote_account: String, keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Load authority keypair
    let authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Derive the whitelist entry PDA
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let (whitelist_entry_pda, _bump) =
        Pubkey::find_program_address(&[vote_account_pubkey.as_ref()], &program_id);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nRemoving validator from whitelist:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!("  Authority:       {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = RemoveFromWhitelistBuilder::new()
        .payer(authority_keypair.pubkey())
        .whitelist_authority(authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .vote_account_pubkey(vote_account_pubkey)
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
    println!("\nValidator removed successfully!");

    Ok(())
}

pub fn update_start_epoch(vote_account: String, epoch: u64, keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Load authority keypair
    let authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Derive the whitelist entry PDA
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let (whitelist_entry_pda, _bump) =
        Pubkey::find_program_address(&[vote_account_pubkey.as_ref()], &program_id);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nUpdating validator start epoch:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!("  New Start Epoch: {}", epoch);
    println!("  Authority:       {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = UpdateStartEpochBuilder::new()
        .payer(authority_keypair.pubkey())
        .whitelist_authority(authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .new_start_epoch(epoch.to_le_bytes())
        .vote_account_pubkey(vote_account_pubkey)
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
    println!("\nStart epoch updated successfully!");

    Ok(())
}

pub fn update_end_epoch(vote_account: String, epoch: u64, keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Load authority keypair
    let authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Derive the whitelist entry PDA
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let (whitelist_entry_pda, _bump) =
        Pubkey::find_program_address(&[vote_account_pubkey.as_ref()], &program_id);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nUpdating validator end epoch:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!(
        "  New End Epoch:   {}",
        if epoch == u64::MAX {
            "∞".to_string()
        } else {
            epoch.to_string()
        }
    );
    println!("  Authority:       {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = UpdateEndEpochBuilder::new()
        .payer(authority_keypair.pubkey())
        .whitelist_authority(authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .new_end_epoch(epoch.to_le_bytes())
        .vote_account_pubkey(vote_account_pubkey)
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
    println!("\nEnd epoch updated successfully!");

    Ok(())
}

pub fn propose_authority(new_authority: String, keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Parse new authority pubkey
    let new_authority_pubkey = new_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid new authority pubkey: {}", e))?;

    // Load current authority keypair
    let authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nProposing authority transfer:");
    println!("  Whitelist Account:   {}", whitelist_pubkey);
    println!("  Current Authority:   {}", authority_keypair.pubkey());
    println!("  New Authority:       {}", new_authority_pubkey);

    // Build the instruction
    let instruction = InitiateAuthorityTransferBuilder::new()
        .payer(authority_keypair.pubkey())
        .whitelist_authority(authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
        .new_authority(new_authority_pubkey)
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
    println!("\nAuthority transfer proposed successfully!");
    println!("\nThe new authority must accept the transfer using:");
    println!("  spherenet-admin vw accept-authority --keypair <new_authority_keypair>");

    Ok(())
}

pub fn accept_authority(keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Load new authority keypair
    let new_authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load new authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nAccepting authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  New Authority:     {}", new_authority_keypair.pubkey());

    // Build the instruction
    let instruction = AcceptAuthorityTransferBuilder::new()
        .payer(new_authority_keypair.pubkey())
        .new_whitelist_authority(new_authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
        .instruction();

    // Get recent blockhash and create transaction
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&new_authority_keypair.pubkey()),
        &[&new_authority_keypair],
        recent_blockhash,
    );

    // Send transaction
    println!("\nSending transaction...");
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("  Signature: {}", signature);
    println!("\nAuthority transfer accepted successfully!");
    println!("You are now the whitelist authority.");

    Ok(())
}

pub fn cancel_authority(keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Load current authority keypair
    let authority_keypair = read_keypair_file(&keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            keypair_path,
            e
        )
    })?;

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nCancelling authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = CancelAuthorityTransferBuilder::new()
        .payer(authority_keypair.pubkey())
        .whitelist_authority(authority_keypair.pubkey())
        .validator_whitelist(whitelist_pubkey)
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
    println!("\nAuthority transfer cancelled successfully!");

    Ok(())
}
