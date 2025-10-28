use crate::consts::RPC_URL;
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{read_keypair_file, Signer};

pub fn airdrop(keypair_path: String, amount: f64) -> Result<()> {
    // Connect to testnet
    let rpc_client = RpcClient::new(RPC_URL);

    // Load keypair
    let keypair = read_keypair_file(&keypair_path)
        .map_err(|e| eyre::eyre!("Failed to load keypair from {}: {}", keypair_path, e))?;

    let pubkey = keypair.pubkey();

    // Check current balance
    let balance = rpc_client.get_balance(&pubkey)?;
    println!("\nRequesting airdrop:");
    println!("  Account:         {}", pubkey);
    println!(
        "  Current Balance: {} SOL",
        balance as f64 / 1_000_000_000.0
    );
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
