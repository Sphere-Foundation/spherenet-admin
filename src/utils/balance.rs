//! Show the native (SPHR) balance of an account.
//!
//! Output is a single line `<amount> SPHR` so the amount is the first
//! whitespace-delimited token — easy to parse from scripts (e.g. the infra
//! funding calc reads it with `awk '{print $1}'`).

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use std::str::FromStr;

pub fn balance(rpc_url: &str, pubkey: String) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let pubkey = Pubkey::from_str(&pubkey)
        .map_err(|e| eyre::eyre!("Invalid pubkey '{}': {}", pubkey, e))?;

    let lamports = rpc_client.get_balance(&pubkey)?;
    println!("{} SPHR", lamports as f64 / LAMPORTS_PER_SOL as f64);

    Ok(())
}
