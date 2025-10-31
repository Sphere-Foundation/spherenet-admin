use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{read_keypair_file, Signer};

pub fn airdrop(rpc_url: &str, keypair_path: String, amount: f64) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Load keypair
    let keypair = read_keypair_file(&keypair_path)
        .map_err(|e| eyre::eyre!("Failed to load keypair from {}: {}", keypair_path, e))?;

    let pubkey = keypair.pubkey();

    // Check current balance
    println!("\nRequesting airdrop:");
    println!("  Account:         {}", pubkey);
    println!("  Airdrop Amount:  {} SOL", amount);

    // Request airdrop
    let lamports = (amount * 1_000_000_000.0) as u64;
    let signature = rpc_client.request_airdrop(&pubkey, lamports)?;

    println!("\nConfirming airdrop...");
    rpc_client.confirm_transaction(&signature)?;

    // Check new balance
    println!("  Signature:       {}", signature);
    println!("\nAirdrop successful!");

    Ok(())
}
