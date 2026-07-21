//! Show the program whitelist account and its deployer authorities.

use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spherenet_program_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ProgramWhitelistAccount, load, whitelist_entry::ProgramWhitelistEntry},
};

#[derive(serde::Serialize)]
pub struct DeployerView {
    deploy_authority: String,
    /// Lifecycle state of the entry: `Pending` (requested, awaiting approval) or
    /// `Approved` (active — may deploy/upgrade).
    state: String,
}

/// Human label for the `EntryState` byte carried on each whitelist entry
/// (`Uninitialized` / `Pending` / `Approved`).
fn entry_state_label(state: u8) -> String {
    match state {
        0 => "Uninitialized".to_string(),
        1 => "Pending".to_string(),
        2 => "Approved".to_string(),
        other => format!("Unknown({other})"),
    }
}

#[derive(serde::Serialize)]
pub struct ProgramWhitelistView {
    program_id: String,
    account: String,
    authority: String,
    pending_authority: String,
    deployers: Vec<DeployerView>,
}

impl Render for ProgramWhitelistView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Program Whitelist Account");
        out.push_str(newline());
        out.push_str(&field("Program ID", &self.program_id));
        out.push_str(&field("Whitelist Account", &self.account));
        out.push_str(newline());
        out.push_str(&field("Authority", &self.authority));
        out.push_str(&field("Pending Authority", &self.pending_authority));
        out.push_str(newline());
        out.push_str(&format!(
            "Whitelisted Deployer Authorities ({}):",
            self.deployers.len()
        ));
        out.push_str(newline());
        if self.deployers.is_empty() {
            out.push_str("  (none)");
        } else {
            for d in &self.deployers {
                out.push_str(&subfield("Deploy Authority", &d.deploy_authority));
                out.push_str(&subfield("State", &d.state));
            }
        }
        out
    }
}

/// Fetch the program whitelist account and its deployer authorities. Shared by
/// the `show` command and the HTTP API — no printing, just data.
pub fn fetch(rpc_url: &str) -> eyre::Result<ProgramWhitelistView> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ProgramWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize program whitelist account: {:?}", e))?;

    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let mut deployers = Vec::new();
    for (pubkey, account) in rpc_client.get_program_accounts(&program_id)? {
        if pubkey == whitelist_pubkey {
            continue; // skip the main whitelist account itself
        }
        if let Ok(entry) = load::<ProgramWhitelistEntry>(&account.data) {
            deployers.push(DeployerView {
                deploy_authority: Pubkey::from(entry.entry_address).to_string(),
                state: entry_state_label(entry.state),
            });
        }
    }

    Ok(ProgramWhitelistView {
        program_id: program_id.to_string(),
        account: whitelist_pubkey.to_string(),
        authority: Pubkey::from(whitelist.authority).to_string(),
        pending_authority: Pubkey::from(whitelist.pending_authority).to_string(),
        deployers,
    })
}

pub fn show(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    emit(&fetch(rpc_url)?, mode)
}
