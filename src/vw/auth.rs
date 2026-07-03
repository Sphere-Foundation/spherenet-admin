use crate::cli::authority::Authority;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_validator_whitelist_client::instructions::{
    AcceptAuthorityTransferBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder,
};
use spherenet_validator_whitelist_interface::account_solana;

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

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nProposing authority transfer:");
    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("  Whitelist Account:   {}", whitelist_pubkey);
    println!("  Current Authority:   {}", instruction_authority);
    println!("  New Authority:       {}", new_authority_pubkey);

    // Build the instruction
    let instruction = InitiateAuthorityTransferBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .new_authority(new_authority_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Propose authority transfer to {}", new_authority_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    println!("\nThe new authority must accept the transfer using:");
    println!("  spherenet-admin vw accept-authority --authority <new_authority_keypair>");

    Ok(())
}

pub fn accept_authority(rpc_url: &str, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nAccepting authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  New Authority:     {}", instruction_authority);

    // Build the instruction
    let instruction = AcceptAuthorityTransferBuilder::new()
        .payer(instruction_authority)
        .new_whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = String::from("Accept authority transfer");
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn cancel_authority(rpc_url: &str, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nCancelling authority transfer:");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", instruction_authority);

    // Build the instruction
    let instruction = CancelAuthorityTransferBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = String::from("Cancel authority transfer");
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}
