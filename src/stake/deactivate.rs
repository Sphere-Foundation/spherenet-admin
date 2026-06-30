//! Deactivate a delegated stake account
//!
//! Begins undelegation of a delegated stake account. Deactivation takes effect
//! over the cooldown period (the stake stays effective through the rest of the
//! current epoch, then cools down). Only the stake authority (staker) signs.
//! After cooldown the balance can be withdrawn by the withdraw authority.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spherenet_stake_interface::{instruction::deactivate_stake, state::StakeStateV2};
use std::str::FromStr;

/// Deactivate a delegated stake account.
///
/// * `stake_account` - pubkey of the delegated stake account.
/// * `stake_authority_path` - keypair of the staker; signs the deactivation.
/// * `payer_path`    - keypair that pays transaction fees.
pub fn deactivate(
    rpc_url: &str,
    stake_account: String,
    stake_authority_path: String,
    payer_path: String,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let stake_pubkey = Pubkey::from_str(&stake_account)
        .map_err(|e| eyre::eyre!("Invalid stake account pubkey '{}': {}", stake_account, e))?;

    let stake_authority = read_keypair_file(&stake_authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read stake authority keypair from {}: {}",
            stake_authority_path,
            e
        )
    })?;
    let payer = read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer keypair from {}: {}", payer_path, e))?;

    // Preflight: must be a delegated stake account authorized for this staker.
    preflight(&rpc_client, &stake_pubkey, &stake_authority.pubkey())?;

    println!("\nDeactivating stake:");
    println!("  Stake Account:   {}", stake_pubkey);
    println!("  Stake Authority: {}", stake_authority.pubkey());
    println!("  Fee Payer:       {}", payer.pubkey());

    let instruction = deactivate_stake(&stake_pubkey, &stake_authority.pubkey());

    let signers = crate::utils::signers::dedupe(&[&payer, &stake_authority]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&signers, rpc_client.get_latest_blockhash()?);
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    println!("\n✅ Stake deactivation submitted!");
    println!("   Stake Account: {}", stake_pubkey);
    println!("   Signature:     {}", signature);
    println!(
        "\n   Stake cools down over the rest of this epoch; withdraw with the\n   \
         withdraw authority once it is fully inactive (`stake show` to track)."
    );

    Ok(())
}

/// The stake account must exist, be stake-program-owned, currently delegated,
/// and authorized for the provided staker.
fn preflight(rpc_client: &RpcClient, stake_pubkey: &Pubkey, staker: &Pubkey) -> eyre::Result<()> {
    let stake_program = Pubkey::from(spherenet_stake_interface::program::id().to_bytes());
    let account = rpc_client.get_account(stake_pubkey).map_err(|_| {
        eyre::eyre!("Stake account {} not found", stake_pubkey)
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
        StakeStateV2::RewardsPool => {
            Err(eyre::eyre!("{} is a rewards pool", stake_pubkey))
        }
    }
}
