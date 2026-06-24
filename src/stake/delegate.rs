//! Delegate a stake account to a vote account
//!
//! Delegates an existing, initialized stake account to a vote account. The
//! whole delegatable balance (account balance − rent reserve) is staked; there
//! is no amount argument. Only the stake authority (staker) signs.
//!
//! SphereNet gates delegation on the validator whitelist: `DelegateStake`
//! carries the vote account's whitelist-entry PDA, and the stake program
//! rejects delegation to a non-whitelisted vote account. We preflight that
//! (and basic account state) so failures surface clearly before fees are spent.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_stake_interface::{instruction::delegate_stake, state::StakeStateV2};
use std::str::FromStr;

/// Delegate an existing stake account to a vote account.
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
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;
    let vote_pubkey = Pubkey::from_str(&vote_account)
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey '{}': {}", vote_account, e))?;

    let stake_authority = read_keypair_file(&stake_authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read stake authority keypair from {}: {}",
            stake_authority_path,
            e
        )
    })?;
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair from {}: {}", payer_path, e))?;

    // ── Preflight ────────────────────────────────────────────────────────────
    preflight_vote_account(&rpc_client, &vote_pubkey)?;
    let (whitelist_entry, _bump) = crate::vw::whitelist::derive_whitelist_entry(&vote_pubkey);
    preflight_whitelisted(&rpc_client, &vote_pubkey, &whitelist_entry)?;
    preflight_stake_account(&rpc_client, &stake_pubkey, &stake_authority.pubkey())?;

    println!("\nDelegating stake:");
    println!("  Stake Account:   {}", stake_pubkey);
    println!("  Vote Account:    {}", vote_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry);
    println!("  Stake Authority: {}", stake_authority.pubkey());
    println!("  Fee Payer:       {}", payer.pubkey());

    let instruction = delegate_stake(
        &stake_pubkey,
        &stake_authority.pubkey(),
        &vote_pubkey,
        &whitelist_entry,
    );

    // Signers: fee payer (payer of the tx) and the staker.
    let signers = crate::utils::signers::dedupe(&[&payer, &stake_authority]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Stake delegated successfully!");
    println!("   Stake Account: {} → Vote Account: {}", stake_pubkey, vote_pubkey);
    println!("   Signature:     {}", signature);

    Ok(())
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
        StakeStateV2::RewardsPool => {
            Err(eyre::eyre!("{} is a rewards pool, not delegatable", stake_pubkey))
        }
    }
}
