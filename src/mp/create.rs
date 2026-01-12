use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey, signature::Signer, signer::keypair::read_keypair_file, transaction::Transaction,
};
use spherenet_monetary_policy_client::instructions::CreateMonetaryPolicyBuilder;
use spherenet_monetary_policy_interface::account_solana;
use std::sync::LazyLock;

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(Pubkey::default);

pub fn create(rpc_url: &str, authority_path: String, payer_path: String) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Load keypairs
    let authority = read_keypair_file(&authority_path)
        .map_err(|e| eyre::eyre!("Failed to read authority keypair: {}", e))?;
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair: {}", e))?;

    println!("\n🏦 Creating monetary policy account...");
    println!("  Authority: {}", authority.pubkey());
    println!("  Payer:     {}", payer.pubkey());

    // Get the monetary policy account address
    let monetary_policy_pubkey = Pubkey::from(account_solana::id().to_bytes());
    println!("  Account:   {}", monetary_policy_pubkey);

    // Build the instruction
    let instruction = CreateMonetaryPolicyBuilder::new()
        .monetary_policy_account(monetary_policy_pubkey)
        .monetary_policy_authority(authority.pubkey())
        .funder(payer.pubkey())
        .system_program(*SYSTEM_PROGRAM)
        .instruction();

    // Create and send transaction
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        recent_blockhash,
    );

    println!("\n📤 Sending transaction...");
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Monetary policy account created successfully!");
    println!("   Account:   {}", monetary_policy_pubkey);
    println!("   Authority: {}", authority.pubkey());
    println!("   Signature: {}", signature);
    println!();

    Ok(())
}
