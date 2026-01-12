use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_authority::Authority;
use spherenet_monetary_policy_client::instructions::{
    AcceptAuthorityTransferBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder,
};
use spherenet_monetary_policy_interface::{
    account_solana,
    state::{account::MonetaryPolicyAccount, load},
};

pub fn auth(rpc_url: &str) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&account_pubkey)?;
    let monetary_policy = load::<MonetaryPolicyAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize monetary policy account: {:?}", e))?;

    // Print authority info
    println!("\nMonetary Policy");
    println!("  Account:           {}", account_pubkey);
    println!("  Authority:         {}", Pubkey::from(monetary_policy.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(monetary_policy.pending_authority)
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

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nProposing authority transfer:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  Current Authority:       {}", instruction_authority);
    println!("  New Authority:           {}", new_authority_pubkey);

    // Build the instruction
    let instruction = InitiateAuthorityTransferBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_authority(new_authority_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Propose authority transfer to {}", new_authority_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn accept_authority(rpc_url: &str, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nAccepting authority transfer:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  New Authority:           {}", instruction_authority);

    // Build the instruction
    let instruction = AcceptAuthorityTransferBuilder::new()
        .monetary_policy_account(account_pubkey)
        .pending_monetary_policy_authority(instruction_authority)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = String::from("Accept authority transfer");
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn cancel_authority(rpc_url: &str, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nCancelling authority transfer:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  Authority:               {}", instruction_authority);

    // Build the instruction
    let instruction = CancelAuthorityTransferBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = String::from("Cancel authority transfer");
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}
