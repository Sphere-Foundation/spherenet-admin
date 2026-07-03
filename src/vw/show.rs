//! Show the validator whitelist account and its entries.

use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spherenet_validator_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ValidatorWhitelistAccount, load, whitelist_entry::ValidatorWhitelistEntry},
};

#[derive(serde::Serialize)]
pub struct WhitelistEntryView {
    vote_account: String,
    start_epoch: u64,
    /// `None` = no end (on-chain sentinel `u64::MAX`, shown as ∞).
    end_epoch: Option<u64>,
}

#[derive(serde::Serialize)]
pub struct WhitelistView {
    program_id: String,
    account: String,
    authority: String,
    pending_authority: String,
    validator_count: u32,
    validators: Vec<WhitelistEntryView>,
}

impl Render for WhitelistView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Validator Whitelist Account");
        out.push_str(newline());
        out.push_str(&field("Program ID", &self.program_id));
        out.push_str(&field("Whitelist Account", &self.account));
        out.push_str(newline());
        out.push_str(&field("Authority", &self.authority));
        out.push_str(&field("Pending Authority", &self.pending_authority));
        out.push_str(newline());
        out.push_str(&format!(
            "Whitelisted Validators ({}):",
            self.validator_count
        ));
        out.push_str(newline());
        if self.validators.is_empty() {
            out.push_str("  (none)");
        } else {
            for e in &self.validators {
                out.push_str(newline());
                out.push_str(&subfield("Vote Account", &e.vote_account));
                out.push_str(&subfield("Start Epoch", e.start_epoch));
                out.push_str(&subfield(
                    "End Epoch",
                    e.end_epoch
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "∞".to_string()),
                ));
            }
        }
        out
    }
}

/// Fetch the validator whitelist account and its entries. Shared by the `show`
/// command and the HTTP API — no printing, just data.
pub fn fetch(rpc_url: &str) -> eyre::Result<WhitelistView> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ValidatorWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize whitelist account: {:?}", e))?;

    let validator_count = u32::from_le_bytes(whitelist.validator_amount);

    // Only scan program accounts when there's something to find.
    let mut validators = Vec::new();
    if validator_count > 0 {
        let program_id = Pubkey::from(program_solana::id().to_bytes());
        for (pubkey, account) in rpc_client.get_program_accounts(&program_id)? {
            if pubkey == whitelist_pubkey {
                continue; // skip the main whitelist account itself
            }
            if let Ok(entry) = load::<ValidatorWhitelistEntry>(&account.data) {
                let end_epoch = u64::from_le_bytes(entry.end_epoch);
                validators.push(WhitelistEntryView {
                    vote_account: Pubkey::from(entry.pubkey).to_string(),
                    start_epoch: u64::from_le_bytes(entry.start_epoch),
                    end_epoch: (end_epoch != u64::MAX).then_some(end_epoch),
                });
            }
        }
    }

    Ok(WhitelistView {
        program_id: Pubkey::from(program_solana::id().to_bytes()).to_string(),
        account: whitelist_pubkey.to_string(),
        authority: Pubkey::from(whitelist.authority).to_string(),
        pending_authority: Pubkey::from(whitelist.pending_authority).to_string(),
        validator_count,
        validators,
    })
}

pub fn show(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    emit(&fetch(rpc_url)?, mode)
}
