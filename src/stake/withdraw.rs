//! Withdraw lamports from a stake account
//!
//! Moves un-delegated (inactive) lamports out of a stake account to a
//! destination. Signed by the **withdraw authority** (not the staker). Only
//! lamports that are not effectively staked can be withdrawn; withdrawing the
//! full balance closes the account. Active/activating stake must be
//! deactivated and cooled down first.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_stake_interface::{instruction::withdraw as withdraw_ix, state::StakeStateV2};
use std::str::FromStr;

/// Withdraw from a stake account.
///
/// * `stake_account` - pubkey of the stake account.
/// * `destination`   - pubkey that receives the lamports.
/// * `amount`        - SPHR to withdraw; ignored when `all` is set.
/// * `all`           - withdraw the entire balance (closes the account).
/// * `withdraw_authority_path` - keypair of the withdraw authority; signs.
/// * `payer_path`    - keypair that pays transaction fees.
pub fn withdraw(
    rpc_url: &str,
    stake_account: String,
    destination: String,
    amount: Option<f64>,
    all: bool,
    withdraw_authority_path: String,
    payer_path: String,
) -> eyre::Result<()> {
    if amount.is_none() && !all {
        return Err(eyre::eyre!("Provide either --amount <SPHR> or --all"));
    }

    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;
    let destination = Pubkey::from_str(&destination)
        .map_err(|e| eyre::eyre!("Invalid destination pubkey '{}': {}", destination, e))?;

    let withdrawer = read_keypair_file(&withdraw_authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read withdraw authority keypair from {}: {}",
            withdraw_authority_path,
            e
        )
    })?;
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair from {}: {}", payer_path, e))?;

    // Preflight: account exists, stake-program owned, withdrawer matches; warn
    // if still delegated. Also resolves the balance for `--all`.
    let balance = preflight(&rpc_client, &stake_pubkey, &withdrawer.pubkey())?;

    let lamports = if all {
        balance
    } else {
        (amount.unwrap() * LAMPORTS_PER_SOL as f64) as u64
    };
    if lamports == 0 {
        return Err(eyre::eyre!("Nothing to withdraw (amount resolves to 0)"));
    }
    if lamports > balance {
        return Err(eyre::eyre!(
            "Requested {:.9} SPHR exceeds the account balance of {:.9} SPHR",
            lamports as f64 / LAMPORTS_PER_SOL as f64,
            balance as f64 / LAMPORTS_PER_SOL as f64
        ));
    }

    println!("\nWithdrawing from stake account:");
    println!("  Stake Account:     {}", stake_pubkey);
    println!("  Destination:       {}", destination);
    println!(
        "  Amount:            {:.9} SPHR{}",
        lamports as f64 / LAMPORTS_PER_SOL as f64,
        if all { " (entire balance — closes account)" } else { "" }
    );
    println!("  Withdraw Authority:{}", withdrawer.pubkey());
    println!("  Fee Payer:         {}", payer.pubkey());

    let instruction = withdraw_ix(
        &stake_pubkey,
        &withdrawer.pubkey(),
        &destination,
        lamports,
        None,
    );

    let signers = crate::utils::signers::dedupe(&[&payer, &withdrawer]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Withdrawal successful!");
    println!("   Signature: {}", signature);
    if all {
        println!("   Stake account {} closed.", stake_pubkey);
    }

    Ok(())
}

/// Validate the account and authority; return the account balance (lamports).
fn preflight(
    rpc_client: &RpcClient,
    stake_pubkey: &Pubkey,
    withdrawer: &Pubkey,
) -> eyre::Result<u64> {
    let stake_program = Pubkey::from(spherenet_stake_interface::program::id().to_bytes());
    let account = rpc_client
        .get_account(stake_pubkey)
        .map_err(|_| eyre::eyre!("Stake account {} not found", stake_pubkey))?;
    if account.owner != stake_program {
        return Err(eyre::eyre!(
            "{} is not a stake account (owner: {})",
            stake_pubkey,
            account.owner
        ));
    }

    let state: StakeStateV2 = bincode::deserialize(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize stake account state: {}", e))?;

    let account_withdrawer = match &state {
        StakeStateV2::Initialized(meta) => Pubkey::from(meta.authorized.withdrawer.to_bytes()),
        StakeStateV2::Stake(meta, _, _) => Pubkey::from(meta.authorized.withdrawer.to_bytes()),
        StakeStateV2::Uninitialized => {
            return Err(eyre::eyre!(
                "Stake account {} is uninitialized — nothing to withdraw",
                stake_pubkey
            ))
        }
        StakeStateV2::RewardsPool => {
            return Err(eyre::eyre!("{} is a rewards pool", stake_pubkey))
        }
    };

    if account_withdrawer != *withdrawer {
        return Err(eyre::eyre!(
            "Provided withdraw authority {} does not match the account's withdrawer {}",
            withdrawer,
            account_withdrawer
        ));
    }

    // Advisory: if delegated and not deactivated, the effective stake is not
    // withdrawable — surface it rather than letting the on-chain error confuse.
    if let StakeStateV2::Stake(_, stake, _) = &state {
        if stake.delegation.deactivation_epoch == u64::MAX {
            println!(
                "\n⚠️  WARNING: this stake account is still delegated and not deactivated.\n   \
                 Only lamports above the effective stake (+ rent) are withdrawable.\n   \
                 Run `stake deactivate` and wait for cooldown to withdraw the full balance."
            );
        }
    }

    Ok(account.lamports)
}
