//! Create a vote account
//!
//! Builds and submits a vote-account creation transaction (v1 `VoteInit` path).
//!
//! Each role is an explicit, independent input — `spherenet-admin` does not
//! collapse them. Callers that want one key to play several roles pass the same
//! keypair/pubkey for each; duplicate *signers* are deduplicated automatically.

use crate::cli::output::{emit, progress, subfield, OutputMode, Render, TxOutputView};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use solana_vote_interface::{
    instruction::{create_account_with_config, withdraw as withdraw_ix, CreateVoteAccountConfig},
    state::VoteInit,
};
use std::str::FromStr;

/// Result of `vote create` — the new account address and creation signature.
#[derive(serde::Serialize)]
pub struct VoteAccountCreatedView {
    vote_account: String,
    signature: String,
}

impl Render for VoteAccountCreatedView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Vote account created\n");
        out.push_str(&subfield("Vote Account", &self.vote_account));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

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
    mode: OutputMode,
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
    let authorized_voter = Pubkey::from_str(&authorized_voter).map_err(|e| {
        eyre::eyre!(
            "Invalid authorized voter pubkey '{}': {}",
            authorized_voter,
            e
        )
    })?;
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

    progress("Creating vote account:");
    progress(format!("Vote Account:    {}", vote_account.pubkey()));
    progress(format!("Identity (node): {}", identity.pubkey()));
    progress(format!("Auth Voter:      {}", authorized_voter));
    progress(format!("Auth Withdrawer: {}", authorized_withdrawer));
    progress(format!("Commission:      {}%", commission));
    progress(format!("Funder (from):   {}", from.pubkey()));
    progress(format!("Fee Payer:       {}", payer.pubkey()));
    progress(format!(
        "Rent reserve:    {:.9} SPHR ({} lamports, {} bytes)",
        rent as f64 / LAMPORTS_PER_SOL as f64,
        rent,
        config.space
    ));

    // The vote account's withdraw authority can drain it. Setting it equal to
    // the validator identity (a hot key on the validator host) is a known
    // footgun; warn but do not block — separation of roles is the caller's call.
    if authorized_withdrawer == identity.pubkey() {
        progress(
            "⚠️  WARNING: authorized withdrawer equals the validator identity. The identity \
             key is hot on the validator host and can drain the vote account. Prefer a \
             distinct, cold withdraw authority.",
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
    let signers = crate::utils::run::dedupe_signers(&[&payer, &from, &vote_account, &identity]);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    emit(
        &VoteAccountCreatedView {
            vote_account: vote_account.pubkey().to_string(),
            signature: signature.to_string(),
        },
        mode,
    )
}

/// Withdraw lamports from a vote account to a destination.
///
/// Signed by the vote account's **authorized withdrawer** (the drain key).
/// `--all` withdraws the entire balance, which closes the account.
///
/// * `vote_account` - pubkey of the vote account.
/// * `destination`  - pubkey that receives the lamports.
/// * `amount`       - SPHR to withdraw; ignored when `all` is set.
/// * `all`          - withdraw the entire balance (closes the account).
/// * `withdraw_authority_path` - keypair of the authorized withdrawer; signs.
/// * `payer_path`   - keypair that pays transaction fees.
#[allow(clippy::too_many_arguments)]
pub fn withdraw(
    rpc_url: &str,
    vote_account: String,
    destination: String,
    amount: Option<f64>,
    all: bool,
    withdraw_authority_path: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    if amount.is_none() && !all {
        return Err(eyre::eyre!("Provide either --amount <SPHR> or --all"));
    }

    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let vote_pubkey = Pubkey::from_str(&vote_account)
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey '{}': {}", vote_account, e))?;
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

    // Preflight: the account must exist and be a vote account. The on-chain
    // program enforces the withdraw authority. Also resolves the balance for `--all`.
    let vote_program = Pubkey::from(solana_vote_interface::program::id().to_bytes());
    let account = rpc_client.get_account(&vote_pubkey).map_err(|_| {
        eyre::eyre!(
            "Vote account {} not found — create it first (`vote create`)",
            vote_pubkey
        )
    })?;
    if account.owner != vote_program {
        return Err(eyre::eyre!(
            "{} is not a vote account (owner: {})",
            vote_pubkey,
            account.owner
        ));
    }
    let balance = account.lamports;

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

    progress("Withdrawing from vote account:");
    progress(format!("Vote Account:      {}", vote_pubkey));
    progress(format!("Destination:       {}", destination));
    progress(format!(
        "Amount:            {:.9} SPHR{}",
        lamports as f64 / LAMPORTS_PER_SOL as f64,
        if all {
            " (entire balance — closes account)"
        } else {
            ""
        }
    ));
    progress(format!("Withdraw Authority:{}", withdrawer.pubkey()));
    progress(format!("Fee Payer:         {}", payer.pubkey()));

    let instruction = withdraw_ix(&vote_pubkey, &withdrawer.pubkey(), lamports, &destination);

    let signers = crate::utils::run::dedupe_signers(&[&payer, &withdrawer]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    if all {
        progress(format!("Vote account {} closed.", vote_pubkey));
    }
    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}
