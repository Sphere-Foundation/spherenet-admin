//! Feature-gate mutations: stage (`activate`) and `revoke` a gate on-chain.
//!
//! `activate` creates the gate's Feature-program account with `activated_at:
//! None` (pending) — the runtime stamps it active at the next epoch boundary, so
//! this call only *stages*; the flip is not immediate. `revoke` destroys a
//! still-pending account (lamports → incinerator) and only works before the gate
//! activates. Both are signed by the gate keypair (`--gate`, a keypair file or a
//! `kms://` URI) plus a fee-payer (`--payer`).

use crate::authority::signer::{dedupe_signers, load_signer};
use crate::cli::output::{emit, progress, subfield, OutputMode, Render, TxOutputView};
use crate::feature::parse_feature;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_feature_gate_interface::{
    instruction::{activate_with_lamports, revoke_pending_activation},
    state::Feature,
};
use solana_sdk::{pubkey::Pubkey, signature::Signer, transaction::Transaction};

/// Result of `feature activate` — the staged gate and its signature.
#[derive(serde::Serialize)]
pub struct FeatureStagedView {
    feature: String,
    signature: String,
}

impl Render for FeatureStagedView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Feature gate staged (pending activation)\n");
        out.push_str(&subfield("Feature", &self.feature));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

/// Stage (activate) a feature gate: create its Feature-program account pending.
/// The runtime activates it at the next epoch boundary — this only stages it.
///
/// * `gate_path`  - the feature keypair (file or `kms://`); signs its own account creation.
/// * `payer_path` - keypair (file or `kms://`) that funds rent + pays fees.
pub fn activate(
    rpc_url: &str,
    gate_path: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let gate = load_signer(&gate_path, "--gate")?;
    let payer = load_signer(&payer_path, "--payer")?;
    let feature_id = gate.pubkey();

    // The gate account must not already exist: on-chain creation fails if it
    // does, and genesis bakes every known gate active — so "already there" is
    // the common case. Report its precise state instead of a raw on-chain error.
    preflight_absent(&rpc_client, &feature_id)?;

    let lamports = rpc_client.get_minimum_balance_for_rent_exemption(Feature::size_of())?;

    progress("Staging feature gate:");
    progress(format!("Feature:   {}", feature_id));
    progress(format!("Fee Payer: {}", payer.pubkey()));
    progress(format!("Rent:      {} lamports (funded by payer)", lamports));

    let instructions = activate_with_lamports(&feature_id, &payer.pubkey(), lamports);

    // Signers: fee payer + the gate key (signs allocate/assign on its own account).
    let signers = dedupe_signers(&[payer.as_ref(), gate.as_ref()]);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction
        .try_sign(&signers, rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    progress(
        "Staged as PENDING — the runtime activates it at the next epoch boundary. \
         Track with `feature status`.",
    );
    emit(
        &FeatureStagedView {
            feature: feature_id.to_string(),
            signature: signature.to_string(),
        },
        mode,
    )
}

/// The gate account must be absent (never staged). If it exists, report its
/// precise state (already active / already pending / not a feature account).
fn preflight_absent(rpc_client: &RpcClient, feature_id: &Pubkey) -> eyre::Result<()> {
    let account = match rpc_client.get_account(feature_id) {
        Ok(account) => account,
        Err(_) => return Ok(()), // absent — good to stage
    };
    match parse_feature(&account) {
        Some(Feature {
            activated_at: Some(slot),
        }) => Err(eyre::eyre!(
            "Feature {} is already active (since slot {}). Activation is permanent — nothing to do.",
            feature_id,
            slot
        )),
        Some(Feature { activated_at: None }) => Err(eyre::eyre!(
            "Feature {} is already staged (pending activation). Wait for the epoch boundary, or `feature revoke` it.",
            feature_id
        )),
        None => Err(eyre::eyre!(
            "Account {} already exists and is not a feature account (owner: {}) — refusing to touch it.",
            feature_id,
            account.owner
        )),
    }
}

/// Revoke a still-pending feature gate: closes the account (lamports →
/// incinerator). Only valid while pending — an active gate cannot be revoked.
///
/// * `gate_path`  - the feature keypair (file or `kms://`); signs the revocation.
/// * `payer_path` - keypair (file or `kms://`) that pays fees.
pub fn revoke(
    rpc_url: &str,
    gate_path: String,
    payer_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let gate = load_signer(&gate_path, "--gate")?;
    let payer = load_signer(&payer_path, "--payer")?;
    let feature_id = gate.pubkey();

    preflight_pending(&rpc_client, &feature_id)?;

    progress("Revoking pending feature gate:");
    progress(format!("Feature:   {}", feature_id));
    progress(format!("Fee Payer: {}", payer.pubkey()));

    let instruction = revoke_pending_activation(&feature_id);

    // Signers: fee payer + the gate key (feature_id is a required signer).
    let signers = dedupe_signers(&[payer.as_ref(), gate.as_ref()]);

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction
        .try_sign(&signers, rpc_client.get_latest_blockhash()?)
        .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    progress("Pending activation revoked — the account was closed and its lamports incinerated.");
    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}

/// The gate account must exist and be pending (`activated_at: None`).
fn preflight_pending(rpc_client: &RpcClient, feature_id: &Pubkey) -> eyre::Result<()> {
    let account = rpc_client.get_account(feature_id).map_err(|_| {
        eyre::eyre!(
            "Feature {} has no account — nothing to revoke (it was never staged).",
            feature_id
        )
    })?;
    match parse_feature(&account) {
        Some(Feature { activated_at: None }) => Ok(()),
        Some(Feature {
            activated_at: Some(slot),
        }) => Err(eyre::eyre!(
            "Feature {} is already active (since slot {}) — revoke only works while pending.",
            feature_id,
            slot
        )),
        None => Err(eyre::eyre!(
            "Account {} is not a feature account (owner: {}).",
            feature_id,
            account.owner
        )),
    }
}
