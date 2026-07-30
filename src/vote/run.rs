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
    instruction::{
        authorize_checked, create_account_with_config, create_account_with_config_v2,
        withdraw as withdraw_ix, CreateVoteAccountConfig,
    },
    state::{VoteAuthorize, VoteInit, VoteInitV2, VoterWithBLSArgs},
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
/// * `authorized_voter`  - keypair permitted to submit votes. Optional — defaults
///   to the identity keypair (identity == voter). It's a *keypair*, not a pubkey,
///   because on the legacy path it must sign the `authorize_checked` that appends
///   the BLS key; its keypair is also what the BLS voter key is derived from.
/// * `authorized_withdrawer` - pubkey permitted to withdraw from the vote account.
///   Optional — defaults to the authorized voter (which itself defaults to the
///   identity), so it trips the hot-key warning unless a distinct cold key is
///   given. Stays a pubkey (not a keypair) because it never signs at create time.
/// * `commission`        - inflation-rewards commission, 0-100.
/// * `from_path`         - keypair that funds the vote account's rent-exempt
///   reserve. Optional — defaults to the identity keypair.
/// * `payer_path`        - keypair that pays transaction fees. Optional — defaults
///   to the identity keypair.
#[allow(clippy::too_many_arguments)]
pub fn create(
    rpc_url: &str,
    vote_account_path: String,
    identity_path: String,
    authorized_voter: Option<String>,
    authorized_withdrawer: Option<String>,
    commission: u8,
    from_path: Option<String>,
    payer_path: Option<String>,
    vote_init_v2: bool,
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
    // Funder and fee payer both default to the identity keypair when omitted.
    let from_path = from_path.unwrap_or_else(|| identity_path.clone());
    let from = read_keypair_file(&from_path)
        .map_err(|e| eyre::eyre!("Failed to read from keypair from {}: {}", from_path, e))?;
    let payer_path = payer_path.unwrap_or_else(|| identity_path.clone());
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair from {}: {}", payer_path, e))?;

    // The authorized voter is loaded as a *keypair* (not a pubkey): on the legacy
    // path it must sign the `authorize_checked` that appends the BLS key, and its
    // keypair is what the BLS voter key is derived from. It defaults to the
    // identity keypair when `--authorized-voter` is omitted — the common validator
    // setup where identity == voter. The withdrawer, by contrast, never signs at
    // create time, so it stays a plain pubkey.
    let authorized_voter_path = authorized_voter.unwrap_or_else(|| identity_path.clone());
    let authorized_voter = read_keypair_file(&authorized_voter_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read authorized voter keypair from {}: {}",
            authorized_voter_path,
            e
        )
    })?;
    // The withdraw authority never signs at create time, so it stays a plain
    // pubkey. Defaults to the authorized VOTER when omitted (which itself defaults
    // to the identity) — so the two vote-account authorities stay unified under the
    // key the caller chose, rather than reaching back to the node identity. Either
    // way this trips the hot-key warning below.
    let authorized_withdrawer = match authorized_withdrawer {
        Some(s) => Pubkey::from_str(&s)
            .map_err(|e| eyre::eyre!("Invalid authorized withdrawer pubkey '{}': {}", s, e))?,
        None => authorized_voter.pubkey(),
    };

    // Compute the rent-exempt reserve for a (V4-sized) vote account. The vote
    // account is funded with exactly this amount: `vote create` creates a
    // rent-exempt account and nothing more. Any additional balance (e.g. for
    // VAT under Alpenglow) is a separate `transfer`.
    let config = CreateVoteAccountConfig::default();
    let rent = rpc_client.get_minimum_balance_for_rent_exemption(config.space as usize)?;

    // Every SphereNet vote account ends up with a BLS voter key — only *how* it
    // gets attached differs, so we always derive the key material here and each
    // arm decides what to do with it. `--vote-init-v2` uses the V2 instruction
    // (VoteInitV2) to set the BLS key at creation, which requires the
    // vote-account-initialize-v2 feature (SIMD-0464) to be active — the V2
    // instruction is rejected otherwise with "invalid instruction data". The
    // default (V1 VoteInit) creates the account without the key; a follow-up
    // `vote authorize-voter-checked` appends this same derived key afterward. Note
    // the stored account is VoteStateV4-layout either way on current builds; the
    // feature only gates whether the BLS key can be populated at init. The BLS key
    // is derived from the authorized VOTER keypair (the on-chain field is
    // `authorized_voter_bls_pubkey`) — which is the identity keypair in the default
    // identity == voter setup.
    let bls = crate::vote::bls::derive_pubkey_and_pop(&authorized_voter, &vote_account.pubkey())?;

    progress("Creating vote account:");
    progress(format!("Vote Account:    {}", vote_account.pubkey()));
    progress(format!("Identity (node): {}", identity.pubkey()));
    progress(format!("Auth Voter:      {}", authorized_voter.pubkey()));
    progress(format!("Auth Withdrawer: {}", authorized_withdrawer));
    progress(format!("Commission:      {}%", commission));
    progress(format!(
        "BLS Pubkey:      {}{}",
        bls.display,
        if vote_init_v2 {
            " (set at init — V2)"
        } else {
            " (append later via `vote authorize-voter-checked`)"
        }
    ));
    progress(format!("Funder (from):   {}", from.pubkey()));
    progress(format!("Fee Payer:       {}", payer.pubkey()));
    progress(format!(
        "Rent reserve:    {:.9} SPHR ({} lamports, {} bytes)",
        rent as f64 / LAMPORTS_PER_SOL as f64,
        rent,
        config.space
    ));

    // The vote account's withdraw authority can drain it. Setting it equal to a
    // hot key that lives on the validator host — the identity OR the authorized
    // voter — is a known footgun; warn but do not block (separation of roles is
    // the caller's call).
    if authorized_withdrawer == identity.pubkey()
        || authorized_withdrawer == authorized_voter.pubkey()
    {
        progress(
            "⚠️  WARNING: authorized withdrawer matches a hot key on the validator host \
             (the validator identity or the authorized voter). That key can drain the vote \
             account. Prefer a distinct, cold withdraw authority.",
        );
    }

    // The two paths share almost nothing once we branch, so we keep them as two
    // self-contained flows — each builds, signs, submits, and reports its own
    // transaction — rather than thread the difference through. Both consume the
    // same `bls` derived above.
    if vote_init_v2 {
        // ── V2: one instruction; BLS set at init (requires SIMD-0464 active) ──
        // `commission` (0-100%) maps to inflation-rewards commission in bps.
        let vote_init = VoteInitV2 {
            node_pubkey: identity.pubkey(),
            authorized_voter: authorized_voter.pubkey(),
            authorized_voter_bls_pubkey: bls.pubkey,
            authorized_voter_bls_proof_of_possession: bls.proof_of_possession,
            authorized_withdrawer,
            inflation_rewards_commission_bps: (commission as u16).saturating_mul(100),
            // Rewards accrue to the vote account; block revenue to the
            // identity — the same defaults the client applies.
            inflation_rewards_collector: vote_account.pubkey(),
            block_revenue_commission_bps: 10_000,
            block_revenue_collector: identity.pubkey(),
        };
        let instructions = create_account_with_config_v2(
            &from.pubkey(),
            &vote_account.pubkey(),
            &vote_init,
            rent,
            config,
        );

        progress("Submitting VoteInitV2 (BLS set at init)...");
        // Signers: fee payer, funder, the vote account (creates itself), and the
        // node identity (signs initialize). The authorized voter does NOT sign at
        // init — its BLS key rides in the instruction data.
        let signers =
            crate::utils::run::dedupe_signers(&[&payer, &from, &vote_account, &identity]);
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
    } else {
        // ── V1: two instructions — legacy VoteInit, then authorize_checked to
        //    APPEND the BLS voter key (VoterWithBLS). This is the lifted
        //    `authorize-voter-checked` body, fed the same `bls` derived above. ──
        let vote_init = VoteInit {
            node_pubkey: identity.pubkey(),
            authorized_voter: authorized_voter.pubkey(),
            authorized_withdrawer,
            commission,
        };
        // create_account_with_config returns [create_account, initialize]; we
        // append the BLS authorize as a third instruction in the same tx.
        let mut instructions = create_account_with_config(
            &from.pubkey(),
            &vote_account.pubkey(),
            &vote_init,
            rent,
            config,
        );

        // authorize_checked requires BOTH the current authorized voter and the
        // new authorized voter to sign. Here we're only ADDING the BLS key (not
        // rotating the voter), so `authorized_voter` plays both roles.
        let append_bls_ix = authorize_checked(
            &vote_account.pubkey(),
            &authorized_voter.pubkey(), // currently authorized voter
            &authorized_voter.pubkey(), // new authorized voter (same key)
            VoteAuthorize::VoterWithBLS(VoterWithBLSArgs {
                bls_pubkey: bls.pubkey,
                bls_proof_of_possession: bls.proof_of_possession,
            }),
        );
        instructions.push(append_bls_ix);

        progress("Submitting VoteInit + authorize_checked(VoterWithBLS)...");
        // The append's checked variant needs the authorized-voter keypair to sign
        // as BOTH the current and new voter (deduped to one signature), so it joins
        // the signer set alongside the payer, funder, vote account, and identity.
        let signers = crate::utils::run::dedupe_signers(&[
            &payer,
            &from,
            &vote_account,
            &identity,
            &authorized_voter,
        ]);
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
