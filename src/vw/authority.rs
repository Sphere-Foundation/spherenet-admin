use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_validator_whitelist_client::instructions::{
    AcceptAuthorityTransferBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder,
};
use spherenet_validator_whitelist_interface::{
    account_solana,
    state::{account::ValidatorWhitelistAccount, load},
};

pub fn auth(rpc_url: &str) -> Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

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

pub fn propose_authority(rpc_url: &str, new_authority: String, authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse new authority pubkey
    let new_authority_pubkey = new_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid new authority pubkey: {}", e))?;

    // Load current authority keypair
    let authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            authority_path,
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

pub fn accept_authority(rpc_url: &str, authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Load new authority keypair
    let new_authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load new authority keypair from {}: {}",
            authority_path,
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

pub fn cancel_authority(rpc_url: &str, authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Load current authority keypair
    let authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            authority_path,
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
