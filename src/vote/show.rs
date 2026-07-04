//! Show a vote account
//!
//! Read-only: fetches a vote account, confirms it is owned by the vote program,
//! deserializes its state (V1_14_11 / V3 / V4), and reports the operator-relevant
//! fields. `fetch` returns `None` when the account does not exist, so callers can
//! render a not-found (CLI) or a 404 (API).

use crate::cli::output::{boxed_header, emit, field, newline, NotFound, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use solana_vote_interface::state::VoteStateVersions;
use std::str::FromStr;

#[derive(serde::Serialize)]
pub struct Credits {
    epoch: u64,
    credits: u64,
}

/// Normalized view across vote-state versions.
#[derive(serde::Serialize)]
pub struct VoteView {
    pubkey: String,
    balance_sphr: f64,
    state_version: String,
    identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    authorized_voter: Option<String>,
    authorized_withdrawer: String,
    commission: String,
    /// Only meaningful for V4 (None for older versions).
    #[serde(skip_serializing_if = "Option::is_none")]
    bls_pubkey_set: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_slot: Option<u64>,
    recent_votes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_credits: Option<Credits>,
}

impl Render for VoteView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Vote Account");
        out.push_str(newline());
        out.push_str(&field("Vote Account", &self.pubkey));
        out.push_str(&field("Balance", format!("{:.9} SPHR", self.balance_sphr)));
        out.push_str(&field("State Version", &self.state_version));
        out.push_str(newline());
        out.push_str(&field("Validator Identity", &self.identity));
        out.push_str(&field(
            "Authorized Voter",
            self.authorized_voter.as_deref().unwrap_or("(none)"),
        ));
        out.push_str(&field("Authorized Withdraw", &self.authorized_withdrawer));
        out.push_str(&field("Commission", &self.commission));
        if let Some(bls) = self.bls_pubkey_set {
            out.push_str(&field("BLS Pubkey", if bls { "set" } else { "(none)" }));
        }
        out.push_str(newline());
        out.push_str(&field(
            "Root Slot",
            self.root_slot
                .map(|s| s.to_string())
                .unwrap_or_else(|| "(none)".to_string()),
        ));
        out.push_str(&field("Recent Votes", self.recent_votes));
        match &self.latest_credits {
            Some(c) => out.push_str(&field(
                "Latest Credits",
                format!("{} (epoch {})", c.credits, c.epoch),
            )),
            None => out.push_str(&field("Latest Credits", "(none — has not voted yet)")),
        }
        out
    }
}

/// Fetch a vote account. `Ok(None)` = account does not exist; `Err` = wrong
/// owner, bad pubkey, deserialize/RPC failure.
pub fn fetch(rpc_url: &str, vote_account: &str) -> eyre::Result<Option<VoteView>> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let vote_pubkey = Pubkey::from_str(vote_account)
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey '{}': {}", vote_account, e))?;

    let account = match rpc_client.get_account(&vote_pubkey) {
        Ok(account) => account,
        Err(_) => return Ok(None),
    };

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

    Ok(Some(build_view(
        &state,
        vote_pubkey.to_string(),
        account.lamports,
    )))
}

pub fn show(rpc_url: &str, vote_account: String, mode: OutputMode) -> eyre::Result<()> {
    match fetch(rpc_url, &vote_account)? {
        Some(view) => emit(&view, mode),
        None => emit(&NotFound::new("Vote account", vote_account), mode),
    }
}

/// Collapse any supported vote-state version into the display view.
fn build_view(state: &VoteStateVersions, pubkey: String, lamports: u64) -> VoteView {
    let balance_sphr = lamports as f64 / LAMPORTS_PER_SOL as f64;
    let latest = |ec: &[(u64, u64, u64)]| {
        ec.last().map(|(e, c, _)| Credits {
            epoch: *e,
            credits: *c,
        })
    };

    match state {
        VoteStateVersions::V1_14_11(s) => VoteView {
            pubkey,
            balance_sphr,
            state_version: "V1_14_11".to_string(),
            identity: Pubkey::from(s.node_pubkey.to_bytes()).to_string(),
            authorized_voter: s
                .authorized_voters
                .last()
                .map(|(_, pk)| Pubkey::from(pk.to_bytes()).to_string()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()).to_string(),
            commission: format!("{}%", s.commission),
            bls_pubkey_set: None,
            root_slot: s.root_slot,
            recent_votes: s.votes.len(),
            latest_credits: latest(&s.epoch_credits),
        },
        VoteStateVersions::V3(s) => VoteView {
            pubkey,
            balance_sphr,
            state_version: "V3".to_string(),
            identity: Pubkey::from(s.node_pubkey.to_bytes()).to_string(),
            authorized_voter: s
                .authorized_voters
                .last()
                .map(|(_, pk)| Pubkey::from(pk.to_bytes()).to_string()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()).to_string(),
            commission: format!("{}%", s.commission),
            bls_pubkey_set: None,
            root_slot: s.root_slot,
            recent_votes: s.votes.len(),
            latest_credits: latest(&s.epoch_credits),
        },
        VoteStateVersions::V4(s) => VoteView {
            pubkey,
            balance_sphr,
            state_version: "V4".to_string(),
            identity: Pubkey::from(s.node_pubkey.to_bytes()).to_string(),
            authorized_voter: s
                .authorized_voters
                .last()
                .map(|(_, pk)| Pubkey::from(pk.to_bytes()).to_string()),
            authorized_withdrawer: Pubkey::from(s.authorized_withdrawer.to_bytes()).to_string(),
            commission: format!(
                "{}bps inflation / {}bps block-revenue",
                s.inflation_rewards_commission_bps, s.block_revenue_commission_bps
            ),
            bls_pubkey_set: Some(s.bls_pubkey_compressed.is_some()),
            root_slot: s.root_slot,
            recent_votes: s.votes.len(),
            latest_credits: latest(&s.epoch_credits),
        },
        // Owned by the vote program but not yet initialized — no authorities or
        // voting state to report.
        VoteStateVersions::Uninitialized => VoteView {
            pubkey,
            balance_sphr,
            state_version: "Uninitialized".to_string(),
            identity: "(uninitialized)".to_string(),
            authorized_voter: None,
            authorized_withdrawer: "(uninitialized)".to_string(),
            commission: "(n/a)".to_string(),
            bls_pubkey_set: None,
            root_slot: None,
            recent_votes: 0,
            latest_credits: None,
        },
    }
}
