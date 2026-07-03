//! Utility reads: balance and epoch.

use crate::cli::output::{emit, field, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use std::str::FromStr;

#[derive(serde::Serialize)]
pub struct BalanceView {
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

/// Fetch an account's native (SPHR) balance. Shared by the `balance` command
/// and the HTTP API. Named `fetch_balance` (not `fetch`) since this module also
/// hosts `epoch`.
pub fn fetch_balance(rpc_url: &str, pubkey: String) -> eyre::Result<BalanceView> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let pubkey =
        Pubkey::from_str(&pubkey).map_err(|e| eyre::eyre!("Invalid pubkey '{}': {}", pubkey, e))?;

    let lamports = rpc_client.get_balance(&pubkey)?;
    Ok(BalanceView {
        pubkey: pubkey.to_string(),
        lamports,
        sphr: lamports as f64 / LAMPORTS_PER_SOL as f64,
    })
}

/// Show the native (SPHR) balance of an account.
pub fn balance(rpc_url: &str, pubkey: String, mode: OutputMode) -> eyre::Result<()> {
    emit(&fetch_balance(rpc_url, pubkey)?, mode)
}

#[derive(serde::Serialize)]
pub struct EpochView {
    epoch: u64,
    slot_index: u64,
    slots_in_epoch: u64,
    absolute_slot: u64,
}

impl Render for EpochView {
    fn to_text(&self) -> String {
        let pct = if self.slots_in_epoch > 0 {
            self.slot_index as f64 / self.slots_in_epoch as f64 * 100.0
        } else {
            0.0
        };
        let mut out = field("Epoch", self.epoch);
        out.push_str(&field(
            "Slot Index",
            format!(
                "{} / {} ({:.2}%)",
                self.slot_index, self.slots_in_epoch, pct
            ),
        ));
        out.push_str(&field("Absolute Slot", self.absolute_slot));
        out
    }
}

/// Fetch the current epoch info. Shared by the `epoch` command and the HTTP API.
pub fn fetch_epoch(rpc_url: &str) -> eyre::Result<EpochView> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let info = rpc_client.get_epoch_info()?;
    Ok(EpochView {
        epoch: info.epoch,
        slot_index: info.slot_index,
        slots_in_epoch: info.slots_in_epoch,
        absolute_slot: info.absolute_slot,
    })
}

/// Show the current epoch.
pub fn epoch(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    emit(&fetch_epoch(rpc_url)?, mode)
}
