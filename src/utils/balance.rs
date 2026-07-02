//! Show the native (SPHR) balance of an account.

use crate::cli::output::{emit, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use std::str::FromStr;

#[derive(serde::Serialize)]
struct BalanceView {
    pubkey: String,
    lamports: u64,
    sphr: f64,
}

impl Render for BalanceView {
    fn to_text(&self) -> String {
        // Single line `<amount> SPHR` — amount is the first whitespace token,
        // for easy parsing in text mode (`awk '{print $1}'`).
        format!("{} SPHR", self.sphr)
    }
}

pub fn balance(rpc_url: &str, pubkey: String, mode: OutputMode) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let pubkey = Pubkey::from_str(&pubkey)
        .map_err(|e| eyre::eyre!("Invalid pubkey '{}': {}", pubkey, e))?;

    let lamports = rpc_client.get_balance(&pubkey)?;
    let view = BalanceView {
        pubkey: pubkey.to_string(),
        lamports,
        sphr: lamports as f64 / LAMPORTS_PER_SOL as f64,
    };
    emit(&view, mode)
}
