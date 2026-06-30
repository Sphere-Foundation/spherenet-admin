//! Create a stake account
//!
//! Builds and submits a stake-account creation transaction: a `system
//! create_account` (funded from `--from`) followed by the stake program's
//! `Initialize`, which records the staker and withdrawer authorities.
//!
//! No vote account is involved here — delegation is a separate step
//! (`stake delegate`). The authorities are set as plain pubkeys and do NOT
//! sign at creation; only the new stake-account key and the funder sign.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_stake_interface::{
    instruction::create_account,
    state::{Authorized, Lockup, StakeStateV2},
};
use std::str::FromStr;

/// Create and initialize a stake account.
///
/// # Roles (all independent)
/// * `stake_account_path`  - keypair of the new stake account; signs its own creation.
/// * `amount`              - SPHR deposited into the account (total; delegatable = amount − rent reserve).
/// * `stake_authority`     - pubkey set as the staker (signs `delegate`/`deactivate` later).
/// * `withdraw_authority`  - pubkey set as the withdrawer (signs withdrawals later).
/// * `from_path`           - keypair that funds the deposited SPHR.
/// * `payer_path`          - keypair that pays transaction fees.
pub fn create(
    rpc_url: &str,
    stake_account_path: String,
    amount: f64,
    stake_authority: String,
    withdraw_authority: String,
    from_path: String,
    payer_path: String,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Load signing keypairs.
    let stake_account = read_keypair_file(&stake_account_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read stake account keypair from {}: {}",
            stake_account_path,
            e
        )
    })?;
    let from = read_keypair_file(&from_path)
        .map_err(|e| eyre::eyre!("Failed to read from keypair from {}: {}", from_path, e))?;
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair from {}: {}", payer_path, e))?;

    // Parse non-signing authority pubkeys.
    let staker = Pubkey::from_str(&stake_authority)
        .map_err(|e| eyre::eyre!("Invalid stake authority pubkey '{}': {}", stake_authority, e))?;
    let withdrawer = Pubkey::from_str(&withdraw_authority).map_err(|e| {
        eyre::eyre!(
            "Invalid withdraw authority pubkey '{}': {}",
            withdraw_authority,
            e
        )
    })?;

    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    // The deposit must at least cover the rent-exempt reserve, or on-chain
    // creation fails. Surface that early with a clear amount.
    let rent = rpc_client.get_minimum_balance_for_rent_exemption(StakeStateV2::size_of())?;
    if lamports < rent {
        return Err(eyre::eyre!(
            "--amount {} SPHR is below the rent-exempt minimum of {:.9} SPHR for a stake account",
            amount,
            rent as f64 / LAMPORTS_PER_SOL as f64
        ));
    }
    let delegatable = lamports.saturating_sub(rent);

    let authorized = Authorized {
        staker,
        withdrawer,
    };
    let lockup = Lockup::default();

    println!("\nCreating stake account:");
    println!("  Stake Account:     {}", stake_account.pubkey());
    println!(
        "  Funding:           {:.9} SPHR ({} lamports)",
        lamports as f64 / LAMPORTS_PER_SOL as f64,
        lamports
    );
    println!(
        "    Rent reserve:    {:.9} SPHR",
        rent as f64 / LAMPORTS_PER_SOL as f64
    );
    println!(
        "    Delegatable:     {:.9} SPHR (staked on delegate)",
        delegatable as f64 / LAMPORTS_PER_SOL as f64
    );
    println!("  Stake Authority:   {}", staker);
    println!("  Withdraw Authority:{}", withdrawer);
    println!("  Funder (from):     {}", from.pubkey());
    println!("  Fee Payer:         {}", payer.pubkey());

    let instructions = create_account(
        &from.pubkey(),
        &stake_account.pubkey(),
        &authorized,
        &lockup,
        lamports,
    );

    // Signers: fee payer first, plus funder and the new account. Authorities
    // are pubkeys only — they do not sign at creation.
    let signers = crate::utils::signers::dedupe(&[&payer, &from, &stake_account]);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Stake account created successfully!");
    println!("   Stake Account: {}", stake_account.pubkey());
    println!("   Signature:     {}", signature);
    println!("\n   Not delegated yet — run `stake delegate` to delegate to a vote account.");

    Ok(())
}
