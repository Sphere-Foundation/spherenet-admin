use crate::consts::RPC_URL;
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_program_whitelist_client::instructions::{
    AcceptAuthorityTransferBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder,
};
use spherenet_program_whitelist_interface::{
    account_solana,
    state::{account::ProgramWhitelistAccount, load},
};

pub fn auth() -> Result<()> {
    // Connect to testnet
    let rpc_client = RpcClient::new(RPC_URL);

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ProgramWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize program whitelist account: {:?}", e))?;

    // Print authority info
    println!("\nProgram Whitelist");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", Pubkey::from(whitelist.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(whitelist.pending_authority)
    );
    println!();

    Ok(())
}

pub fn propose_authority(new_authority: String, authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

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

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nProposing authority transfer:");
    println!("  Whitelist Account:   {}", whitelist_pubkey);
    println!("  Current Authority:   {}", authority_keypair.pubkey());
    println!("  New Authority:       {}", new_authority_pubkey);

    // Build the instruction
    let instruction = InitiateAuthorityTransferBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(authority_keypair.pubkey())
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
    println!("  spherenet-admin pw accept-authority --keypair <new_authority_keypair>");

    Ok(())
}

pub fn accept_authority(authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Load new authority keypair
    let new_authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load new authority keypair from {}: {}",
            authority_path,
            e
        )
    })?;

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nAccepting authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  New Authority:     {}", new_authority_keypair.pubkey());

    // Build the instruction
    let instruction = AcceptAuthorityTransferBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .pending_whitelist_authority(new_authority_keypair.pubkey())
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

pub fn cancel_authority(authority_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(RPC_URL);

    // Load current authority keypair
    let authority_keypair = read_keypair_file(&authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to load authority keypair from {}: {}",
            authority_path,
            e
        )
    })?;

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nCancelling authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", authority_keypair.pubkey());

    // Build the instruction
    let instruction = CancelAuthorityTransferBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(authority_keypair.pubkey())
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
