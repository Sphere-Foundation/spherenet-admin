//! Show a feature gate's activation status (read-only).
//!
//! Fetches the Feature account at the gate pubkey and reports the tri-state:
//! **absent** (never staged) / **pending** (`activated_at: None`) / **active**
//! (`activated_at: Some(slot)`). `absent` is a normal result, not an error, so
//! it renders as a view rather than a not-found.

use crate::authority::kms;
use crate::cli::output::{boxed_header, emit, field, newline, OutputMode, Render};
use crate::feature::parse_feature;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_feature_gate_interface::state::Feature;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[derive(serde::Serialize)]
pub struct FeatureView {
    pubkey: String,
    /// absent | pending | active
    state: String,
    is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    activated_at_slot: Option<u64>,
}

impl Render for FeatureView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Feature Gate");
        out.push_str(newline());
        out.push_str(&field("Feature", &self.pubkey));
        out.push_str(&field("State", &self.state));
        out.push_str(&field("Active", self.is_active));
        if let Some(slot) = self.activated_at_slot {
            out.push_str(&field("Activated At Slot", slot));
        }
        out
    }
}

/// Fetch a feature gate's status. `absent` / `pending` / `active` are all `Ok`;
/// `Err` only on a bad identifier, an RPC failure, or an account that exists but
/// is not owned by the Feature program.
pub fn fetch(rpc_url: &str, gate: &str) -> eyre::Result<FeatureView> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let feature_id = resolve_gate_pubkey(gate)?;

    let account = match rpc_client.get_account(&feature_id) {
        Ok(account) => account,
        Err(_) => {
            return Ok(FeatureView {
                pubkey: feature_id.to_string(),
                state: "absent".to_string(),
                is_active: false,
                activated_at_slot: None,
            })
        }
    };

    match parse_feature(&account) {
        Some(Feature {
            activated_at: Some(slot),
        }) => Ok(FeatureView {
            pubkey: feature_id.to_string(),
            state: "active".to_string(),
            is_active: true,
            activated_at_slot: Some(slot),
        }),
        Some(Feature { activated_at: None }) => Ok(FeatureView {
            pubkey: feature_id.to_string(),
            state: "pending".to_string(),
            is_active: false,
            activated_at_slot: None,
        }),
        None => Err(eyre::eyre!(
            "Account {} exists but is not a feature account (owner: {})",
            feature_id,
            account.owner
        )),
    }
}

pub fn show(rpc_url: &str, gate: String, mode: OutputMode) -> eyre::Result<()> {
    emit(&fetch(rpc_url, &gate)?, mode)
}

/// Resolve a gate identifier to its pubkey: a base58 pubkey, or a `kms://` URI
/// (we read the `?pubkey=` it embeds — no KMS client needed for a read).
fn resolve_gate_pubkey(gate: &str) -> eyre::Result<Pubkey> {
    if gate.starts_with(kms::KMS_URI_SCHEME) {
        let pk = gate
            .split("pubkey=")
            .nth(1)
            .ok_or_else(|| eyre::eyre!("kms:// URI is missing '?pubkey=': {gate}"))?;
        Pubkey::from_str(pk).map_err(|e| eyre::eyre!("Invalid pubkey in kms:// URI '{gate}': {e}"))
    } else {
        Pubkey::from_str(gate).map_err(|e| eyre::eyre!("Invalid gate pubkey '{gate}': {e}"))
    }
}
