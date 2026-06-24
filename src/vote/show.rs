//! Show a vote account
//!
//! Read-only: fetches a vote account, confirms it is owned by the vote program,
//! deserializes its state (V1_14_11 / V3 / V4), and prints the operator-relevant
//! fields. Useful as an existence/idempotency check and for inspecting the
//! authorities set by `vote create`.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use solana_vote_interface::state::VoteStateVersions;
use std::str::FromStr;

/// Normalized view across vote-state versions for display.
struct VoteView {
    node_pubkey: Pubkey,
    authorized_withdrawer: Pubkey,
    /// Most recent authorized voter (latest epoch entry), if any.
    authorized_voter: Option<Pubkey>,
    commission: String,
    num_votes: usize,
    root_slot: Option<u64>,
    /// Latest (epoch, credits) entry, if any.
    latest_credits: Option<(u64, u64)>,
    /// BLS pubkey presence — only meaningful for V4 (None for older versions).
    bls_present: Option<bool>,
    version: &'static str,
}

pub fn show(rpc_url: &str, vote_account: String) -> eyre::Result<()> {
    // `confirmed` so freshly created accounts are visible immediately (the
    // create commands confirm at this level); finalized would lag ~13s.
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let vote_pubkey = Pubkey::from_str(&vote_account)
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey '{}': {}", vote_account, e))?;

    let account = match rpc_client.get_account(&vote_pubkey) {
        Ok(account) => account,
        Err(_) => {
            println!("\nVote account {} not found.", vote_pubkey);
            return Ok(());
        }
    };

    // Confirm ownership by the vote program before trying to deserialize.
    let vote_program = Pubkey::from(solana_vote_interface::program::id().to_bytes());
    if account.owner != vote_program {
        return Err(eyre::eyre!(
            "Account {} is not a vote account (owner: {}, expected: {})",
            vote_pubkey,
            account.owner,
            vote_program
        ));
    }

    let state = VoteStateVersions::deserialize(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize vote account state: {:?}", e))?;

    let view = normalize(&state);

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                       Vote Account                            ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Vote Account:        {}", vote_pubkey);
    println!(
        "Balance:             {:.9} SPHR",
        account.lamports as f64 / LAMPORTS_PER_SOL as f64
    );
    println!("State Version:       {}", view.version);
    println!();
    println!("Validator Identity:  {}", view.node_pubkey);
    println!(
        "Authorized Voter:    {}",
        view.authorized_voter
            .map(|pk| pk.to_string())
            .unwrap_or_else(|| "(none)".to_string())
    );
    println!("Authorized Withdraw: {}", view.authorized_withdrawer);
    println!("Commission:          {}", view.commission);
    if let Some(bls) = view.bls_present {
        println!(
            "BLS Pubkey:          {}",
            if bls { "set" } else { "(none)" }
        );
    }
    println!();
    println!(
        "Root Slot:           {}",
        view.root_slot
            .map(|s| s.to_string())
            .unwrap_or_else(|| "(none)".to_string())
    );
    println!("Recent Votes:        {}", view.num_votes);
    match view.latest_credits {
        Some((epoch, credits)) => {
            println!("Latest Credits:      {} (epoch {})", credits, epoch);
        }
        None => println!("Latest Credits:      (none — has not voted yet)"),
    }
    println!();

    Ok(())
}

/// Collapse any supported vote-state version into a common display view.
fn normalize(state: &VoteStateVersions) -> VoteView {
    match state {
        VoteStateVersions::V1_14_11(s) => VoteView {
            node_pubkey: Pubkey::from(s.node_pubkey.to_bytes()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()),
            authorized_voter: s
                .authorized_voters
                .last()
                .map(|(_, pk)| Pubkey::from(pk.to_bytes())),
            commission: format!("{}%", s.commission),
            num_votes: s.votes.len(),
            root_slot: s.root_slot,
            latest_credits: s.epoch_credits.last().map(|(e, c, _)| (*e, *c)),
            bls_present: None,
            version: "V1_14_11",
        },
        VoteStateVersions::V3(s) => VoteView {
            node_pubkey: Pubkey::from(s.node_pubkey.to_bytes()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()),
            authorized_voter: s
                .authorized_voters
                .last()
                .map(|(_, pk)| Pubkey::from(pk.to_bytes())),
            commission: format!("{}%", s.commission),
            num_votes: s.votes.len(),
            root_slot: s.root_slot,
            latest_credits: s.epoch_credits.last().map(|(e, c, _)| (*e, *c)),
            bls_present: None,
            version: "V3",
        },
        VoteStateVersions::V4(s) => VoteView {
            node_pubkey: Pubkey::from(s.node_pubkey.to_bytes()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()),
            authorized_voter: s
                .authorized_voters
                .last()
                .map(|(_, pk)| Pubkey::from(pk.to_bytes())),
            commission: format!(
                "{}bps inflation / {}bps block-revenue",
                s.inflation_rewards_commission_bps, s.block_revenue_commission_bps
            ),
            num_votes: s.votes.len(),
            root_slot: s.root_slot,
            latest_credits: s.epoch_credits.last().map(|(e, c, _)| (*e, *c)),
            bls_present: Some(s.bls_pubkey_compressed.is_some()),
            version: "V4",
        },
        VoteStateVersions::V0_23_5(s) => VoteView {
            node_pubkey: Pubkey::from(s.node_pubkey.to_bytes()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()),
            authorized_voter: Some(Pubkey::from(s.authorized_voter.to_bytes())),
            commission: format!("{}%", s.commission),
            num_votes: s.votes.len(),
            root_slot: s.root_slot,
            latest_credits: s.epoch_credits.last().map(|(e, c, _)| (*e, *c)),
            bls_present: None,
            version: "V0_23_5",
        },
    }
}
