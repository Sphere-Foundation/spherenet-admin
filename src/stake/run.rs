//! Stake account operations: create, delegate, deactivate, withdraw.
//!
//! All keypair-file based; the relevant authority signs each operation.
//! `create` sets authorities as pubkeys (no signature); `delegate`/`deactivate`
//! are signed by the staker; `withdraw` by the withdraw authority.
//!
//! `deactivate --force` is the exception: it is permissionless (only the fee
//! payer signs) and deactivates stake left delegated to a validator that has
//! been removed from the validator whitelist.

use crate::cli::output::{emit, progress, subfield, OutputMode, Render, TxOutputView};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Signer, transaction::Transaction,
};
use spherenet_stake_interface::{
    instruction::{
        create_account, deactivate_delinquent_stake, deactivate_stake, delegate_stake,
        withdraw as withdraw_ix,
    },
    state::{Authorized, Lockup, StakeStateV2},
};
use std::str::FromStr;

/// Result of `stake create` — the new account address and creation signature.
#[derive(serde::Serialize)]
pub struct StakeAccountCreatedView {
    stake_account: String,
    signature: String,
}

impl Render for StakeAccountCreatedView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Stake account created\n");
        out.push_str(&subfield("Stake Account", &self.stake_account));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

// ══════════════════════════════════════════════════════════════════════════ //
//                                  CREATE                                      //
// ══════════════════════════════════════════════════════════════════════════ //

/// Create and initialize a stake account (does not delegate).
///
/// * `stake_account_path` - keypair of the new stake account; signs its own creation.
/// * `amount`             - SPHR deposited (total; delegatable = amount − rent reserve).
/// * `stake_authority`    - pubkey set as the staker (signs `delegate`/`deactivate` later).
/// * `withdraw_authority` - pubkey set as the withdrawer (signs withdrawals later).
/// * `from_path`          - keypair that funds the deposited SPHR.
/// * `payer_path`         - keypair that pays transaction fees.
#[allow(clippy::too_many_arguments)]
pub fn create(
    rpc_url: &str,
    stake_account_path: String,
    amount: f64,
    stake_authority: String,
    withdraw_authority: String,
    from_path: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Load signing keypairs.
    let stake_account =
        crate::utils::run::read_keypair_file_checked(&stake_account_path, "--stake-account")?;
    let from = crate::utils::run::read_keypair_file_checked(&from_path, "--from")?;
    let payer = crate::utils::run::read_keypair_file_checked(&payer_path, "--payer")?;

    // Parse non-signing authority pubkeys.
    let staker = Pubkey::from_str(&stake_authority).map_err(|e| {
        eyre::eyre!(
            "Invalid stake authority pubkey '{}': {}",
            stake_authority,
            e
        )
    })?;
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

    let authorized = Authorized { staker, withdrawer };
    let lockup = Lockup::default();

    progress("Creating stake account:");
    progress(format!("Stake Account:     {}", stake_account.pubkey()));
    progress(format!(
        "Funding:           {:.9} SPHR ({} lamports)",
        lamports as f64 / LAMPORTS_PER_SOL as f64,
        lamports
    ));
    progress(format!(
        "  Rent reserve:    {:.9} SPHR",
        rent as f64 / LAMPORTS_PER_SOL as f64
    ));
    progress(format!(
        "  Delegatable:     {:.9} SPHR (staked on delegate)",
        delegatable as f64 / LAMPORTS_PER_SOL as f64
    ));
    progress(format!("Stake Authority:   {}", staker));
    progress(format!("Withdraw Authority:{}", withdrawer));
    progress(format!("Funder (from):     {}", from.pubkey()));
    progress(format!("Fee Payer:         {}", payer.pubkey()));

    let instructions = create_account(
        &from.pubkey(),
        &stake_account.pubkey(),
        &authorized,
        &lockup,
        lamports,
    );

    // Signers: fee payer first, plus funder and the new account. Authorities
    // are pubkeys only — they do not sign at creation.
    let signers = crate::utils::run::dedupe_signers(&[&payer, &from, &stake_account]);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction
        .try_sign(&signers, rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    progress("Not delegated yet — run `stake delegate` to delegate to a vote account.");
    emit(
        &StakeAccountCreatedView {
            stake_account: stake_account.pubkey().to_string(),
            signature: signature.to_string(),
        },
        mode,
    )
}

// ══════════════════════════════════════════════════════════════════════════ //
//                                 DELEGATE                                     //
// ══════════════════════════════════════════════════════════════════════════ //

/// Delegate an existing stake account to a vote account.
///
/// The whole delegatable balance is staked (no amount argument). Only the stake
/// authority (staker) signs. SphereNet gates delegation on the validator
/// whitelist — preflighted so failures surface before fees are spent.
///
/// * `stake_account` - pubkey of the initialized stake account.
/// * `vote_account`  - pubkey of the (whitelisted) vote account to delegate to.
/// * `stake_authority_path` - keypair of the staker; signs the delegation.
/// * `payer_path`    - keypair that pays transaction fees.
pub fn delegate(
    rpc_url: &str,
    stake_account: String,
    vote_account: String,
    stake_authority_path: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;
    let vote_pubkey = Pubkey::from_str(&vote_account)
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey '{}': {}", vote_account, e))?;

    let stake_authority =
        crate::utils::run::read_keypair_file_checked(&stake_authority_path, "--stake-authority")?;
    let payer = crate::utils::run::read_keypair_file_checked(&payer_path, "--payer")?;

    // ── Preflight ────────────────────────────────────────────────────────────
    preflight_vote_account(&rpc_client, &vote_pubkey)?;
    let (whitelist_entry, _bump) = crate::vw::run::derive_whitelist_entry(&vote_pubkey);
    preflight_whitelisted(&rpc_client, &vote_pubkey, &whitelist_entry)?;
    preflight_stake_account(&rpc_client, &stake_pubkey, &stake_authority.pubkey())?;

    progress("Delegating stake:");
    progress(format!("Stake Account:   {}", stake_pubkey));
    progress(format!("Vote Account:    {}", vote_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry));
    progress(format!("Stake Authority: {}", stake_authority.pubkey()));
    progress(format!("Fee Payer:       {}", payer.pubkey()));

    let instruction = delegate_stake(
        &stake_pubkey,
        &stake_authority.pubkey(),
        &vote_pubkey,
        &whitelist_entry,
    );

    // Signers: fee payer (payer of the tx) and the staker.
    let signers = crate::utils::run::dedupe_signers(&[&payer, &stake_authority]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction
        .try_sign(&signers, rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}

/// The vote account must exist and be owned by the vote program.
fn preflight_vote_account(rpc_client: &RpcClient, vote_pubkey: &Pubkey) -> eyre::Result<()> {
    let vote_program = Pubkey::from(solana_vote_interface::program::id().to_bytes());
    match rpc_client.get_account(vote_pubkey) {
        Ok(account) if account.owner == vote_program => Ok(()),
        Ok(account) => Err(eyre::eyre!(
            "{} is not a vote account (owner: {})",
            vote_pubkey,
            account.owner
        )),
        Err(_) => Err(eyre::eyre!(
            "Vote account {} not found — create it first (`vote create`)",
            vote_pubkey
        )),
    }
}

/// SphereNet gate: the vote account's validator-whitelist entry PDA must exist,
/// or the stake program rejects the delegation.
fn preflight_whitelisted(
    rpc_client: &RpcClient,
    vote_pubkey: &Pubkey,
    whitelist_entry: &Pubkey,
) -> eyre::Result<()> {
    if rpc_client.get_account(whitelist_entry).is_err() {
        return Err(eyre::eyre!(
            "Vote account {} is not whitelisted (no entry at {}).\n\
             Delegation will be rejected by the stake program. Run:\n  \
             spherenet-admin vw add {} --authority <whitelist-authority>",
            vote_pubkey,
            whitelist_entry,
            vote_pubkey
        ));
    }
    Ok(())
}

/// The stake account must exist, be stake-program-owned, initialized, not
/// already delegated, and authorized for the provided staker.
fn preflight_stake_account(
    rpc_client: &RpcClient,
    stake_pubkey: &Pubkey,
    staker: &Pubkey,
) -> eyre::Result<()> {
    let stake_program = Pubkey::from(spherenet_stake_interface::program::id().to_bytes());
    let account = rpc_client.get_account(stake_pubkey).map_err(|_| {
        eyre::eyre!(
            "Stake account {} not found — create it first (`stake create`)",
            stake_pubkey
        )
    })?;
    if account.owner != stake_program {
        return Err(eyre::eyre!(
            "{} is not a stake account (owner: {})",
            stake_pubkey,
            account.owner
        ));
    }

    let state: StakeStateV2 = bincode::deserialize(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize stake account state: {}", e))?;

    match state {
        StakeStateV2::Initialized(meta) => {
            let account_staker = Pubkey::from(meta.authorized.staker.to_bytes());
            if account_staker != *staker {
                return Err(eyre::eyre!(
                    "Provided stake authority {} does not match the account's staker {}",
                    staker,
                    account_staker
                ));
            }
            Ok(())
        }
        StakeStateV2::Stake(..) => Err(eyre::eyre!(
            "Stake account {} is already delegated (run `stake show` to inspect)",
            stake_pubkey
        )),
        StakeStateV2::Uninitialized => Err(eyre::eyre!(
            "Stake account {} is uninitialized — create it with `stake create`",
            stake_pubkey
        )),
        StakeStateV2::RewardsPool => Err(eyre::eyre!(
            "{} is a rewards pool, not delegatable",
            stake_pubkey
        )),
    }
}

// ══════════════════════════════════════════════════════════════════════════ //
//                                DEACTIVATE                                    //
// ══════════════════════════════════════════════════════════════════════════ //

/// Deactivate a delegated stake account (begins cooldown).
///
/// Only the stake authority (staker) signs. After cooldown the balance can be
/// withdrawn by the withdraw authority.
///
/// * `stake_account` - pubkey of the delegated stake account.
/// * `stake_authority_path` - keypair of the staker; signs the deactivation.
/// * `payer_path`    - keypair that pays transaction fees.
pub fn deactivate(
    rpc_url: &str,
    stake_account: String,
    stake_authority_path: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;

    let stake_authority =
        crate::utils::run::read_keypair_file_checked(&stake_authority_path, "--stake-authority")?;
    let payer = crate::utils::run::read_keypair_file_checked(&payer_path, "--payer")?;

    // Preflight: must be a delegated stake account authorized for this staker.
    preflight_deactivate(&rpc_client, &stake_pubkey, &stake_authority.pubkey())?;

    progress("Deactivating stake:");
    progress(format!("Stake Account:   {}", stake_pubkey));
    progress(format!("Stake Authority: {}", stake_authority.pubkey()));
    progress(format!("Fee Payer:       {}", payer.pubkey()));

    let instruction = deactivate_stake(&stake_pubkey, &stake_authority.pubkey());

    let signers = crate::utils::run::dedupe_signers(&[&payer, &stake_authority]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction
        .try_sign(&signers, rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    progress(
        "Stake cools down over the rest of this epoch; withdraw with the withdraw \
         authority once it is fully inactive (`stake show` to track).",
    );
    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}

/// The stake account must exist, be stake-program-owned, currently delegated,
/// and authorized for the provided staker.
fn preflight_deactivate(
    rpc_client: &RpcClient,
    stake_pubkey: &Pubkey,
    staker: &Pubkey,
) -> eyre::Result<()> {
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

    match state {
        StakeStateV2::Stake(meta, _stake, _flags) => {
            let account_staker = Pubkey::from(meta.authorized.staker.to_bytes());
            if account_staker != *staker {
                return Err(eyre::eyre!(
                    "Provided stake authority {} does not match the account's staker {}",
                    staker,
                    account_staker
                ));
            }
            Ok(())
        }
        StakeStateV2::Initialized(_) => Err(eyre::eyre!(
            "Stake account {} is not delegated — nothing to deactivate",
            stake_pubkey
        )),
        StakeStateV2::Uninitialized => Err(eyre::eyre!(
            "Stake account {} is uninitialized",
            stake_pubkey
        )),
        StakeStateV2::RewardsPool => Err(eyre::eyre!("{} is a rewards pool", stake_pubkey)),
    }
}

/// Force-deactivate stake delegated to a delisted validator.
///
/// SphereNet's stake program lets anyone deactivate a stake account whose
/// delegation points at a vote account removed from the validator whitelist.
/// The vote account is read from the stake account's delegation and the
/// whitelist entry PDA is derived from it.
///
/// * `stake_account` - pubkey of the delegated stake account.
/// * `payer_path`    - keypair that pays transaction fees (only signer).
pub fn force_deactivate(
    rpc_url: &str,
    stake_account: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;

    let payer = crate::utils::run::read_keypair_file_checked(&payer_path, "--payer")?;

    // ── Preflight ────────────────────────────────────────────────────────────
    let vote_pubkey = preflight_delegated_stake(&rpc_client, &stake_pubkey)?;
    let (whitelist_entry, _bump) = crate::vw::run::derive_whitelist_entry(&vote_pubkey);
    preflight_delisted(&rpc_client, &vote_pubkey, &whitelist_entry)?;

    progress("Force-deactivating delisted stake:");
    progress(format!("Stake Account:   {}", stake_pubkey));
    progress(format!("Vote Account:    {}", vote_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry));
    progress(format!("Fee Payer:       {}", payer.pubkey()));

    let instruction = deactivate_delinquent_stake(&stake_pubkey, &vote_pubkey, &whitelist_entry);

    // Signers: fee payer only — the instruction is permissionless.
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction
        .try_sign(&[&payer], rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    progress(
        "Stake cools down over the rest of this epoch; withdraw with the withdraw \
         authority once it is fully inactive (`stake show` to track).",
    );
    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}

/// The stake account must exist, be stake-program-owned, and currently
/// delegated. Returns the vote account the stake is delegated to.
fn preflight_delegated_stake(
    rpc_client: &RpcClient,
    stake_pubkey: &Pubkey,
) -> eyre::Result<Pubkey> {
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

    match state {
        StakeStateV2::Stake(_meta, stake, _flags) => {
            Ok(Pubkey::from(stake.delegation.voter_pubkey.to_bytes()))
        }
        StakeStateV2::Initialized(_) => Err(eyre::eyre!(
            "Stake account {} is not delegated — nothing to deactivate",
            stake_pubkey
        )),
        StakeStateV2::Uninitialized => Err(eyre::eyre!(
            "Stake account {} is uninitialized",
            stake_pubkey
        )),
        StakeStateV2::RewardsPool => Err(eyre::eyre!("{} is a rewards pool", stake_pubkey)),
    }
}

/// Forced deactivation only succeeds if the validator has been removed from the whitelist,
/// its entry PDA must be a closed and no longer program owned, mirroring the on-chain check.
fn preflight_delisted(
    rpc_client: &RpcClient,
    vote_pubkey: &Pubkey,
    whitelist_entry: &Pubkey,
) -> eyre::Result<()> {
    let whitelist_program =
        Pubkey::from(spherenet_validator_whitelist_interface::program_solana::id().to_bytes());
    match rpc_client.get_account(whitelist_entry) {
        // Entry closed and reaped — validator fully removed.
        Err(_) => Ok(()),
        // Tombstone: emptied and returned to the system program.
        Ok(account) if account.data.is_empty() && account.owner != whitelist_program => Ok(()),
        Ok(_) => Err(eyre::eyre!(
            "Vote account {} is still whitelisted (entry at {}).\n\
             Forced deactivation will be rejected by the stake program. Remove it first:\n  \
             spherenet-admin vw remove {} --authority <whitelist-authority>",
            vote_pubkey,
            whitelist_entry,
            vote_pubkey
        )),
    }
}

// ══════════════════════════════════════════════════════════════════════════ //
//                                 WITHDRAW                                     //
// ══════════════════════════════════════════════════════════════════════════ //

/// Withdraw un-delegated (inactive) lamports from a stake account to a
/// destination. Signed by the **withdraw authority** (not the staker).
/// `--all` withdraws the entire balance and closes the account.
///
/// * `stake_account` - pubkey of the stake account.
/// * `destination`   - pubkey that receives the lamports.
/// * `amount`        - SPHR to withdraw; ignored when `all` is set.
/// * `all`           - withdraw the entire balance (closes the account).
/// * `withdraw_authority_path` - keypair of the withdraw authority; signs.
/// * `payer_path`    - keypair that pays transaction fees.
#[allow(clippy::too_many_arguments)]
pub fn withdraw(
    rpc_url: &str,
    stake_account: String,
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

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;
    let destination = Pubkey::from_str(&destination)
        .map_err(|e| eyre::eyre!("Invalid destination pubkey '{}': {}", destination, e))?;

    let withdrawer = crate::utils::run::read_keypair_file_checked(
        &withdraw_authority_path,
        "--withdraw-authority",
    )?;
    let payer = crate::utils::run::read_keypair_file_checked(&payer_path, "--payer")?;

    // Preflight: account exists, stake-program owned, withdrawer matches; warn
    // if still delegated. Also resolves the balance for `--all`.
    let balance = preflight_withdraw(&rpc_client, &stake_pubkey, &withdrawer.pubkey())?;

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

    progress("Withdrawing from stake account:");
    progress(format!("Stake Account:     {}", stake_pubkey));
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

    let instruction = withdraw_ix(
        &stake_pubkey,
        &withdrawer.pubkey(),
        &destination,
        lamports,
        None,
    );

    let signers = crate::utils::run::dedupe_signers(&[&payer, &withdrawer]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction
        .try_sign(&signers, rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    if all {
        progress(format!("Stake account {} closed.", stake_pubkey));
    }
    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}

/// Validate the account and authority; return the account balance (lamports).
fn preflight_withdraw(
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
        StakeStateV2::RewardsPool => return Err(eyre::eyre!("{} is a rewards pool", stake_pubkey)),
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
            progress(
                "⚠️  WARNING: this stake account is still delegated and not deactivated. \
                 Only lamports above the effective stake (+ rent) are withdrawable. Run \
                 `stake deactivate` and wait for cooldown to withdraw the full balance.",
            );
        }
    }

    Ok(account.lamports)
}
