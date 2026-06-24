//! Show a stake account
//!
//! Read-only: fetches a stake account, confirms it is owned by the stake
//! program, bincode-deserializes its `StakeStateV2`, and prints authorities,
//! lockup, and delegation. Doubles as the existence / "already delegated?"
//! check used during validator activation.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use spherenet_stake_interface::state::StakeStateV2;
use std::str::FromStr;

const U64_MAX: u64 = u64::MAX;

pub fn show(rpc_url: &str, stake_account: String) -> eyre::Result<()> {
    // `confirmed` so freshly created accounts are visible immediately (the
    // create commands confirm at this level); finalized would lag ~13s.
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;

    let account = match rpc_client.get_account(&stake_pubkey) {
        Ok(account) => account,
        Err(_) => {
            println!("\nStake account {} not found.", stake_pubkey);
            return Ok(());
        }
    };

    // Confirm ownership by the stake program before deserializing.
    let stake_program = Pubkey::from(spherenet_stake_interface::program::id().to_bytes());
    if account.owner != stake_program {
        return Err(eyre::eyre!(
            "Account {} is not a stake account (owner: {}, expected: {})",
            stake_pubkey,
            account.owner,
            stake_program
        ));
    }

    let state: StakeStateV2 = bincode::deserialize(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize stake account state: {}", e))?;

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                      Stake Account                            ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Stake Account:       {}", stake_pubkey);
    println!(
        "Balance:             {:.9} SPHR",
        account.lamports as f64 / LAMPORTS_PER_SOL as f64
    );

    match state {
        StakeStateV2::Uninitialized => {
            println!("State:               Uninitialized");
            println!("  (account exists but holds no stake state)");
        }
        StakeStateV2::RewardsPool => {
            println!("State:               RewardsPool");
        }
        StakeStateV2::Initialized(meta) => {
            println!("State:               Initialized (not delegated)");
            print_authorities(&meta);
        }
        StakeStateV2::Stake(meta, stake, _flags) => {
            println!("State:               Delegated");
            print_authorities(&meta);

            let current_epoch = rpc_client.get_epoch_info().map(|e| e.epoch).ok();
            let d = &stake.delegation;

            println!();
            println!(
                "Delegated Vote Acct: {}",
                Pubkey::from(d.voter_pubkey.to_bytes())
            );
            println!(
                "Delegated Stake:     {:.9} SPHR",
                d.stake as f64 / LAMPORTS_PER_SOL as f64
            );
            println!("Activation Epoch:    {}", fmt_epoch(d.activation_epoch));
            println!("Deactivation Epoch:  {}", fmt_epoch(d.deactivation_epoch));
            println!("Credits Observed:    {}", stake.credits_observed);
            println!(
                "Status:              {}",
                delegation_status(d.activation_epoch, d.deactivation_epoch, current_epoch)
            );
        }
    }
    println!();

    Ok(())
}

fn print_authorities(meta: &spherenet_stake_interface::state::Meta) {
    println!(
        "Stake Authority:     {}",
        Pubkey::from(meta.authorized.staker.to_bytes())
    );
    println!(
        "Withdraw Authority:  {}",
        Pubkey::from(meta.authorized.withdrawer.to_bytes())
    );

    let lockup = &meta.lockup;
    let custodian = Pubkey::from(lockup.custodian.to_bytes());
    if lockup.unix_timestamp == 0 && lockup.epoch == 0 && custodian == Pubkey::default() {
        println!("Lockup:              none");
    } else {
        println!(
            "Lockup:              until unix {} / epoch {} (custodian {})",
            lockup.unix_timestamp, lockup.epoch, custodian
        );
    }
}

/// Format a sentinel-or-value epoch (`u64::MAX` means "unset").
fn fmt_epoch(epoch: u64) -> String {
    if epoch == U64_MAX {
        "∞ (unset)".to_string()
    } else {
        epoch.to_string()
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
    if activation_epoch == U64_MAX {
        return "active (bootstrap)".to_string();
    }

    match current_epoch {
        Some(now) => {
            if deactivation_epoch != U64_MAX {
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
            // Could not fetch current epoch; report the raw intent.
            if deactivation_epoch != U64_MAX {
                format!("deactivated at epoch {}", deactivation_epoch)
            } else {
                format!("delegated (activated epoch {})", activation_epoch)
            }
        }
    }
}
