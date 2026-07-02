//! Show a stake account
//!
//! Read-only: fetches a stake account, confirms it is owned by the stake
//! program, bincode-deserializes its `StakeStateV2`, and reports authorities,
//! lockup, and delegation. `fetch` returns `None` when the account does not
//! exist, so callers can render a not-found (CLI) or a 404 (API).

use crate::cli::output::{boxed_header, emit, field, newline, NotFound, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use spherenet_stake_interface::state::{Lockup, StakeStateV2};
use std::str::FromStr;

#[derive(serde::Serialize)]
pub struct LockupView {
    unix_timestamp: i64,
    epoch: u64,
    custodian: String,
}

#[derive(serde::Serialize)]
pub struct DelegationView {
    vote_account: String,
    stake_sphr: f64,
    /// `None` = unset (on-chain sentinel `u64::MAX`).
    activation_epoch: Option<u64>,
    deactivation_epoch: Option<u64>,
    credits_observed: u64,
    status: String,
}

#[derive(serde::Serialize)]
pub struct StakeView {
    pubkey: String,
    balance_sphr: f64,
    /// Uninitialized | Initialized | Delegated | RewardsPool
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stake_authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    withdraw_authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lockup: Option<LockupView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegation: Option<DelegationView>,
}

impl Render for StakeView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Stake Account");
        out.push_str(newline());
        out.push_str(&field("Stake Account", &self.pubkey));
        out.push_str(&field("Balance", format!("{:.9} SPHR", self.balance_sphr)));
        out.push_str(&field("State", &self.state));
        if let Some(s) = &self.stake_authority {
            out.push_str(&field("Stake Authority", s));
        }
        if let Some(w) = &self.withdraw_authority {
            out.push_str(&field("Withdraw Authority", w));
        }
        // Lockup line only makes sense for initialized/delegated accounts.
        if self.stake_authority.is_some() {
            let lockup = match &self.lockup {
                Some(l) => format!(
                    "until unix {} / epoch {} (custodian {})",
                    l.unix_timestamp, l.epoch, l.custodian
                ),
                None => "none".to_string(),
            };
            out.push_str(&field("Lockup", lockup));
        }
        if let Some(d) = &self.delegation {
            out.push_str(newline());
            out.push_str(&field("Delegated Vote Acct", &d.vote_account));
            out.push_str(&field("Delegated Stake", format!("{:.9} SPHR", d.stake_sphr)));
            out.push_str(&field("Activation Epoch", fmt_epoch(d.activation_epoch)));
            out.push_str(&field("Deactivation Epoch", fmt_epoch(d.deactivation_epoch)));
            out.push_str(&field("Credits Observed", d.credits_observed));
            out.push_str(&field("Status", &d.status));
        }
        out
    }
}

/// Fetch a stake account. `Ok(None)` = account does not exist; `Err` = wrong
/// owner, bad pubkey, deserialize/RPC failure.
pub fn fetch(rpc_url: &str, stake_account: &str) -> eyre::Result<Option<StakeView>> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;

    let account = match rpc_client.get_account(&stake_pubkey) {
        Ok(account) => account,
        Err(_) => return Ok(None),
    };

    let stake_program = Pubkey::from(spherenet_stake_interface::program::id().to_bytes());
    if account.owner != stake_program {
        return Err(eyre::eyre!(
            "Account {} is not a stake account (owner: {})",
            stake_pubkey,
            account.owner
        ));
    }

    let state: StakeStateV2 = bincode::deserialize(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize stake account state: {}", e))?;

    let pubkey = stake_pubkey.to_string();
    let balance_sphr = account.lamports as f64 / LAMPORTS_PER_SOL as f64;

    let view = match state {
        StakeStateV2::Uninitialized => StakeView {
            pubkey,
            balance_sphr,
            state: "Uninitialized".to_string(),
            stake_authority: None,
            withdraw_authority: None,
            lockup: None,
            delegation: None,
        },
        StakeStateV2::RewardsPool => StakeView {
            pubkey,
            balance_sphr,
            state: "RewardsPool".to_string(),
            stake_authority: None,
            withdraw_authority: None,
            lockup: None,
            delegation: None,
        },
        StakeStateV2::Initialized(meta) => StakeView {
            pubkey,
            balance_sphr,
            state: "Initialized".to_string(),
            stake_authority: Some(Pubkey::from(meta.authorized.staker.to_bytes()).to_string()),
            withdraw_authority: Some(
                Pubkey::from(meta.authorized.withdrawer.to_bytes()).to_string(),
            ),
            lockup: lockup_view(&meta.lockup),
            delegation: None,
        },
        StakeStateV2::Stake(meta, stake, _flags) => {
            let current_epoch = rpc_client.get_epoch_info().map(|e| e.epoch).ok();
            let d = &stake.delegation;
            StakeView {
                pubkey,
                balance_sphr,
                state: "Delegated".to_string(),
                stake_authority: Some(Pubkey::from(meta.authorized.staker.to_bytes()).to_string()),
                withdraw_authority: Some(
                    Pubkey::from(meta.authorized.withdrawer.to_bytes()).to_string(),
                ),
                lockup: lockup_view(&meta.lockup),
                delegation: Some(DelegationView {
                    vote_account: Pubkey::from(d.voter_pubkey.to_bytes()).to_string(),
                    stake_sphr: d.stake as f64 / LAMPORTS_PER_SOL as f64,
                    activation_epoch: opt_epoch(d.activation_epoch),
                    deactivation_epoch: opt_epoch(d.deactivation_epoch),
                    credits_observed: stake.credits_observed,
                    status: delegation_status(
                        d.activation_epoch,
                        d.deactivation_epoch,
                        current_epoch,
                    ),
                }),
            }
        }
    };
    Ok(Some(view))
}

pub fn show(rpc_url: &str, stake_account: String, mode: OutputMode) -> eyre::Result<()> {
    match fetch(rpc_url, &stake_account)? {
        Some(view) => emit(&view, mode),
        None => emit(&NotFound::new("Stake account", stake_account), mode),
    }
}

/// `u64::MAX` is the "unset" sentinel for activation/deactivation epochs.
fn opt_epoch(epoch: u64) -> Option<u64> {
    (epoch != u64::MAX).then_some(epoch)
}

fn fmt_epoch(epoch: Option<u64>) -> String {
    match epoch {
        Some(e) => e.to_string(),
        None => "∞ (unset)".to_string(),
    }
}

fn lockup_view(lockup: &Lockup) -> Option<LockupView> {
    let custodian = Pubkey::from(lockup.custodian.to_bytes());
    if lockup.unix_timestamp == 0 && lockup.epoch == 0 && custodian == Pubkey::default() {
        None
    } else {
        Some(LockupView {
            unix_timestamp: lockup.unix_timestamp,
            epoch: lockup.epoch,
            custodian: custodian.to_string(),
        })
    }
}

/// Coarse delegation status. Precise effective stake needs the stake-history
/// sysvar (warmup/cooldown); this is a best-effort label from the epochs alone.
fn delegation_status(
    activation_epoch: u64,
    deactivation_epoch: u64,
    current_epoch: Option<u64>,
) -> String {
    // Bootstrap stake activates immediately and never warms up.
    if activation_epoch == u64::MAX {
        return "active (bootstrap)".to_string();
    }

    match current_epoch {
        Some(now) => {
            if deactivation_epoch != u64::MAX {
                if now >= deactivation_epoch {
                    "inactive (deactivated)".to_string()
                } else {
                    format!("deactivating (until epoch {})", deactivation_epoch)
                }
            } else if now <= activation_epoch {
                format!("activating (since epoch {})", activation_epoch)
            } else {
                format!("active (since epoch {})", activation_epoch)
            }
        }
        None => {
            if deactivation_epoch != u64::MAX {
                format!("deactivated at epoch {}", deactivation_epoch)
            } else {
                format!("delegated (activated epoch {})", activation_epoch)
            }
        }
    }
}
