//! Multisig read command.
//!
//! `fetch` resolves + validates the multisig by its create-key and returns a
//! serde [`MultisigView`]; `show` renders it via the shared output layer. A
//! bogus create-key fails in `squads::resolve` rather than rendering not-found.

use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use crate::squads::{self, Permissions};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

#[derive(serde::Serialize)]
pub struct MemberView {
    key: String,
    permissions: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct MultisigView {
    create_key: String,
    multisig_pda: String,
    vault_pda: String,
    vault_balance_sphr: f64,
    threshold: u16,
    transaction_index: u64,
    stale_transaction_index: u64,
    time_lock: u32,
    /// `None` = autonomous (no external config authority).
    #[serde(skip_serializing_if = "Option::is_none")]
    config_authority: Option<String>,
    /// `None` = rent collection disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    rent_collector: Option<String>,
    members: Vec<MemberView>,
}

impl Render for MultisigView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Multisig Vault");
        out.push_str(newline());
        out.push_str(&field("Create Key", &self.create_key));
        out.push_str(&field("Multisig PDA", &self.multisig_pda));
        out.push_str(&field("Vault PDA", &self.vault_pda));
        out.push_str(&field(
            "Vault Balance",
            format!("{:.9} SPHR", self.vault_balance_sphr),
        ));
        out.push_str(&field(
            "Threshold",
            format!("{}/{}", self.threshold, self.members.len()),
        ));
        out.push_str(&field("Next TX Index", self.transaction_index));
        out.push_str(&field("Stale TX Index", self.stale_transaction_index));
        out.push_str(&field("Time Lock", format!("{} seconds", self.time_lock)));
        out.push_str(&field(
            "Config Authority",
            self.config_authority.as_deref().unwrap_or("Autonomous"),
        ));
        out.push_str(&field(
            "Rent Collector",
            self.rent_collector.as_deref().unwrap_or("Disabled"),
        ));
        out.push_str(newline());
        out.push_str(&format!("Members ({}):\n", self.members.len()));
        for (i, m) in self.members.iter().enumerate() {
            let perms = if m.permissions.is_empty() {
                "No permissions".to_string()
            } else {
                m.permissions.join(" | ")
            };
            out.push_str(&subfield(
                &format!("[{}]", i + 1),
                format!("{} ({})", m.key, perms),
            ));
        }
        out
    }
}

/// Fetch a multisig vault, referenced by its create-key. Resolves + validates
/// via [`squads::resolve`] (a bogus create-key errors there), then builds the
/// view from the returned account — one fetch, no raw PDAs.
pub fn fetch(rpc_url: &str, create_key: Pubkey) -> eyre::Result<MultisigView> {
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let resolved = squads::resolve(&rpc, &create_key)?;
    let m = &resolved.account;

    // Vault (index 0) holds the funds.
    let vault_balance = rpc.get_balance(&resolved.vault).unwrap_or(0);

    let config_authority =
        (m.config_authority != Pubkey::default()).then(|| m.config_authority.to_string());

    let members = m
        .members
        .iter()
        .map(|member| MemberView {
            key: member.key.to_string(),
            permissions: permission_list(&member.permissions),
        })
        .collect();

    Ok(MultisigView {
        create_key: m.create_key.to_string(),
        multisig_pda: resolved.multisig.to_string(),
        vault_pda: resolved.vault.to_string(),
        vault_balance_sphr: vault_balance as f64 / 1_000_000_000.0,
        threshold: m.threshold,
        transaction_index: m.transaction_index,
        stale_transaction_index: m.stale_transaction_index,
        time_lock: m.time_lock,
        config_authority,
        rent_collector: m.rent_collector.map(|rc| rc.to_string()),
        members,
    })
}

/// Show a multisig vault, referenced by its create-key (pubkey).
pub fn show(rpc_url: &str, create_key: String, mode: OutputMode) -> eyre::Result<()> {
    let create_key = create_key
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid create-key '{}': {}", create_key, e))?;
    emit(&fetch(rpc_url, create_key)?, mode)
}

/// The permissions granted to a member, as a list of readable names.
fn permission_list(perms: &Permissions) -> Vec<String> {
    let mut out = Vec::new();
    if perms.mask & (squads::types::Permission::Initiate as u8) != 0 {
        out.push("Initiate".to_string());
    }
    if perms.mask & (squads::types::Permission::Vote as u8) != 0 {
        out.push("Vote".to_string());
    }
    if perms.mask & (squads::types::Permission::Execute as u8) != 0 {
        out.push("Execute".to_string());
    }
    out
}
