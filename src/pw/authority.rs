use crate::cli::Authority;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_program_whitelist_client::instructions::{
    AcceptAuthorityTransferBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder,
};
use spherenet_program_whitelist_interface::{
    account_solana,
    state::{account::ProgramWhitelistAccount, load},
};

pub fn auth(rpc_url: &str) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

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

pub fn propose_authority(
    rpc_url: &str,
    new_authority: String,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse new authority pubkey
    let new_authority_pubkey = new_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid new authority pubkey: {}", e))?;

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nProposing authority transfer:");
    println!("  Whitelist Account:   {}", whitelist_pubkey);
    println!("  Current Authority:   {}", instruction_authority);
    println!("  New Authority:       {}", new_authority_pubkey);

    // Build the instruction
    let instruction = InitiateAuthorityTransferBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(instruction_authority)
        .new_authority(new_authority_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Propose authority transfer to {}", new_authority_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn accept_authority(rpc_url: &str, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nAccepting authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  New Authority:     {}", instruction_authority);

    // Build the instruction
    let instruction = AcceptAuthorityTransferBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .pending_whitelist_authority(instruction_authority)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = String::from("Accept authority transfer");
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn cancel_authority(rpc_url: &str, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nCancelling authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", instruction_authority);

    // Build the instruction
    let instruction = CancelAuthorityTransferBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(instruction_authority)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = String::from("Cancel authority transfer");
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}
