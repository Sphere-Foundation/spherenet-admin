use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub fn airdrop(rpc_url: &str, pubkey_str: String, amount: f64) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse pubkey
    let pubkey = Pubkey::from_str(&pubkey_str)
        .map_err(|e| eyre::eyre!("Failed to parse pubkey {}: {}", pubkey_str, e))?;

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
