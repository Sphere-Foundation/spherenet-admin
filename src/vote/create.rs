//! Create a vote account
//!
//! Builds and submits a vote-account creation transaction (v1 `VoteInit` path).
//!
//! Each role is an explicit, independent input — `spherenet-admin` does not
//! collapse them. Callers that want one key to play several roles pass the same
//! keypair/pubkey for each; duplicate *signers* are deduplicated automatically.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use solana_vote_interface::{
    instruction::{create_account_with_config, CreateVoteAccountConfig},
    state::VoteInit,
};
use std::str::FromStr;

/// Create and initialize a vote account.
///
/// # Roles (all independent)
/// * `vote_account_path` - keypair of the new vote account; signs its own creation.
/// * `identity_path`     - validator identity / node; signs the initialize instruction.
/// * `authorized_voter`  - pubkey permitted to submit votes.
/// * `authorized_withdrawer` - pubkey permitted to withdraw from the vote account.
/// * `commission`        - inflation-rewards commission, 0-100.
/// * `from_path`         - keypair that funds the vote account's rent-exempt reserve.
/// * `payer_path`        - keypair that pays transaction fees.
#[allow(clippy::too_many_arguments)]
pub fn create(
    rpc_url: &str,
    vote_account_path: String,
    identity_path: String,
    authorized_voter: String,
    authorized_withdrawer: String,
    commission: u8,
    from_path: String,
    payer_path: String,
) -> eyre::Result<()> {
    if commission > 100 {
        return Err(eyre::eyre!(
            "Commission must be between 0 and 100 (got {})",
            commission
        ));
    }

    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Load signing keypairs.
    let vote_account = read_keypair_file(&vote_account_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read vote account keypair from {}: {}",
            vote_account_path,
            e
        )
    })?;
    let identity = read_keypair_file(&identity_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read identity keypair from {}: {}",
            identity_path,
            e
        )
    })?;
    let from = read_keypair_file(&from_path)
        .map_err(|e| eyre::eyre!("Failed to read from keypair from {}: {}", from_path, e))?;
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair from {}: {}", payer_path, e))?;

    // Parse non-signing authority pubkeys.
    let authorized_voter = Pubkey::from_str(&authorized_voter)
        .map_err(|e| eyre::eyre!("Invalid authorized voter pubkey '{}': {}", authorized_voter, e))?;
    let authorized_withdrawer = Pubkey::from_str(&authorized_withdrawer).map_err(|e| {
        eyre::eyre!(
            "Invalid authorized withdrawer pubkey '{}': {}",
            authorized_withdrawer,
            e
        )
    })?;

    // Compute the rent-exempt reserve for a (V4-sized) vote account. The vote
    // account is funded with exactly this amount: `vote create` creates a
    // rent-exempt account and nothing more. Any additional balance (e.g. for
    // VAT under Alpenglow) is a separate `transfer`.
    let config = CreateVoteAccountConfig::default();
    let rent = rpc_client.get_minimum_balance_for_rent_exemption(config.space as usize)?;

    let vote_init = VoteInit {
        node_pubkey: identity.pubkey(),
        authorized_voter,
        authorized_withdrawer,
        commission,
    };

    println!("\nCreating vote account:");
    println!("  Vote Account:    {}", vote_account.pubkey());
    println!("  Identity (node): {}", identity.pubkey());
    println!("  Auth Voter:      {}", authorized_voter);
    println!("  Auth Withdrawer: {}", authorized_withdrawer);
    println!("  Commission:      {}%", commission);
    println!("  Funder (from):   {}", from.pubkey());
    println!("  Fee Payer:       {}", payer.pubkey());
    println!(
        "  Rent reserve:    {:.9} SPHR ({} lamports, {} bytes)",
        rent as f64 / LAMPORTS_PER_SOL as f64,
        rent,
        config.space
    );

    // The vote account's withdraw authority can drain it. Setting it equal to
    // the validator identity (a hot key on the validator host) is a known
    // footgun; warn but do not block — separation of roles is the caller's call.
    if authorized_withdrawer == identity.pubkey() {
        println!(
            "\n⚠️  WARNING: authorized withdrawer equals the validator identity.\n   \
             The identity key is hot on the validator host and can drain the vote\n   \
             account. Prefer a distinct, cold withdraw authority."
        );
    }

    let instructions = create_account_with_config(
        &from.pubkey(),
        &vote_account.pubkey(),
        &vote_init,
        rent,
        config,
    );

    // Sign with every required signer, deduplicated by pubkey. The fee payer
    // must come first so it is the transaction's payer.
    let signers = crate::utils::signers::dedupe(&[&payer, &from, &vote_account, &identity]);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Vote account created successfully!");
    println!("   Vote Account: {}", vote_account.pubkey());
    println!("   Signature:    {}", signature);

    Ok(())
}
