//! Utility actions: airdrop, transfer — plus the shared signer-dedup helper
//! used across command modules.

use crate::cli::authority::Authority;
use crate::cli::output::{emit, progress, subfield, OutputMode, Render};
use crate::squads;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
};
use solana_system_interface::instruction as system_instruction;
use std::str::FromStr;

/// Deduplicate signers by pubkey, preserving order (first occurrence wins).
///
/// Several roles (funder, fee payer, authority, new account) frequently
/// collapse onto one keypair. A transaction that lists the same key twice as a
/// signer is rejected, so callers pass every role and let this drop duplicates.
pub fn dedupe_signers<'a>(signers: &[&'a dyn Signer]) -> Vec<&'a dyn Signer> {
    let mut seen: Vec<Pubkey> = Vec::with_capacity(signers.len());
    let mut out: Vec<&dyn Signer> = Vec::with_capacity(signers.len());
    for signer in signers {
        let pk = signer.pubkey();
        if !seen.contains(&pk) {
            seen.push(pk);
            out.push(*signer);
        }
    }
    out
}

/// Load a signer from a CLI value: a keypair file path, or a `kms://` URI for
/// a key held in GCP Cloud KMS (see [`crate::kms`]).
///
/// This is the default loader for every keypair argument; `flag` names the
/// argument for error messages. Arguments that must stay file-based use
/// [`read_keypair_file_checked`] instead.
pub fn load_signer(path: &str, flag: &str) -> eyre::Result<Box<dyn Signer>> {
    if path.starts_with(crate::kms::KMS_URI_SCHEME) {
        Ok(Box::new(crate::kms::KmsSigner::from_uri(path)?))
    } else {
        let keypair = read_keypair_file(path)
            .map_err(|e| eyre::eyre!("Failed to read keypair for {flag} from {path}: {e}"))?;
        Ok(Box::new(keypair))
    }
}

/// Load a keypair for an argument that must stay a local file — the
/// `program deploy`/`upgrade` chunk-signing roles and `vote create`'s
/// identity/authorized-voter keys. Rejects a `kms://` URI with a clear error
/// instead of `read_keypair_file`'s baffling "No such file or directory";
/// `flag` names the argument (and may carry the reason) for error messages.
pub fn read_keypair_file_checked(path: &str, flag: &str) -> eyre::Result<Keypair> {
    if path.starts_with(crate::kms::KMS_URI_SCHEME) {
        eyre::bail!("KMS signers are not supported for {flag}; pass a keypair file path");
    }
    read_keypair_file(path)
        .map_err(|e| eyre::eyre!("Failed to read keypair for {flag} from {path}: {e}"))
}

#[derive(serde::Serialize)]
pub struct AirdropResult {
    pubkey: String,
    amount_sphr: f64,
    signature: String,
    new_balance_sphr: f64,
}

impl Render for AirdropResult {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Airdrop successful!\n");
        out.push_str(&subfield("Account", &self.pubkey));
        out.push_str(&subfield("Amount", format!("{} SPHR", self.amount_sphr)));
        out.push_str(&subfield(
            "New balance",
            format!("{} SPHR", self.new_balance_sphr),
        ));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

/// Request an airdrop and verify it actually landed. Shared by the `airdrop`
/// command and the HTTP API — no printing (beyond progress), returns the result.
pub fn request_airdrop(
    rpc_url: &str,
    pubkey_str: String,
    amount: f64,
) -> eyre::Result<AirdropResult> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let pubkey = Pubkey::from_str(&pubkey_str)
        .map_err(|e| eyre::eyre!("Failed to parse pubkey {}: {}", pubkey_str, e))?;
    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    // Capture the balance up front so we can verify the airdrop actually landed
    // — a signature alone doesn't prove funding (the faucet may be empty, the
    // request rate-limited, or the transaction may fail on-chain).
    let before = rpc_client.get_balance(&pubkey)?;

    progress(format!(
        "Requesting airdrop of {} SPHR to {}...",
        amount, pubkey
    ));
    // Pass a FRESH blockhash. The RPC node's faucet signs the funding transfer
    // with whatever blockhash we hand it; the blockhash-less `request_airdrop`
    // sends `None`, so the faucet falls back to the bank's last confirmed
    // blockhash — often already stale by the time the transfer lands, so it
    // silently never confirms (balance unchanged). Fetching the latest blockhash
    // and requesting against it is what the client CLI does.
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let signature =
        rpc_client.request_airdrop_with_blockhash(&pubkey, lamports, &recent_blockhash)?;

    progress("Confirming airdrop...");
    // Confirm against the SAME fresh blockhash we requested with, retrying until
    // the tx confirms or that blockhash expires — this is what the client CLI
    // does. The single-shot `confirm_transaction` checks exactly once, right
    // after submit, and returns false before a faucet transfer has had time to
    // land, so it loses the race and reports a false "did not confirm".
    rpc_client
        .confirm_transaction_with_spinner(&signature, &recent_blockhash, rpc_client.commitment())
        .map_err(|e| {
            eyre::eyre!(
                "Airdrop {} did not confirm — the faucet may be empty or rate-limited: {}",
                signature,
                e
            )
        })?;

    let after = rpc_client.get_balance(&pubkey)?;
    if after <= before {
        return Err(eyre::eyre!(
            "Airdrop confirmed (signature {}) but the balance did not increase (still {} SPHR) \
             — the faucet could not fund the request.",
            signature,
            after as f64 / LAMPORTS_PER_SOL as f64
        ));
    }

    Ok(AirdropResult {
        pubkey: pubkey.to_string(),
        amount_sphr: amount,
        signature: signature.to_string(),
        new_balance_sphr: after as f64 / LAMPORTS_PER_SOL as f64,
    })
}

/// Request an airdrop for an account (testnet).
pub fn airdrop(
    rpc_url: &str,
    pubkey_str: String,
    amount: f64,
    mode: OutputMode,
) -> eyre::Result<()> {
    emit(&request_airdrop(rpc_url, pubkey_str, amount)?, mode)
}

/// Transfer SPHR to a raw address (`to`) or to a multisig's vault (`to_multisig`,
/// by create-key). Exactly one destination must be given; a multisig create-key
/// is resolved + validated so funds land in the vault, never the config account.
pub fn transfer(
    rpc_url: &str,
    from: Authority,
    to: Option<String>,
    to_multisig: Option<String>,
    amount: f64,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Resolve the destination (raw address or a multisig's vault) through the
    // shared multisig-safety gate.
    let destination = crate::cli::authority::resolve_target(
        &rpc_client,
        to,
        to_multisig,
        "--to",
        "--to-multisig",
    )?;

    // Convert SPHR to lamports
    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    progress(format!("Transferring {} SPHR to {}", amount, destination));

    // Show the source (progress → stderr).
    match &from {
        crate::cli::authority::Authority::SingleSig { signer } => {
            progress(format!("From:     {}", signer.pubkey()));
        }
        crate::cli::authority::Authority::MultiSig { multisig, member } => {
            progress(format!("Proposer: {}", member.pubkey()));
            progress(format!("Multisig: {}", multisig));
            // Funds come from the vault PDA, not the config account.
            let program_id =
                squads::types::SQUADS_PROGRAM_ID.parse::<solana_sdk::pubkey::Pubkey>()?;
            let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);
            let vault_balance = rpc_client.get_balance(&vault_pda).unwrap_or(0);
            progress(format!(
                "Vault:    {} ({:.9} SPHR)",
                vault_pda,
                vault_balance as f64 / LAMPORTS_PER_SOL as f64
            ));
        }
    }

    // Build system transfer instruction (from = vault PDA for multisig).
    let from_pubkey = from.instruction_authority_pubkey()?;
    let instruction = system_instruction::transfer(&from_pubkey, &destination, lamports);

    let description = format!("Transfer {} SPHR to {}", amount, destination);
    let result = from.execute_instruction(&rpc_client, instruction, &description)?;

    emit(&result, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::write_keypair_file;

    fn temp_keypair_file(name: &str) -> (Keypair, String) {
        let keypair = Keypair::new();
        let path = std::env::temp_dir().join(format!(
            "spherenet-admin-utils-test-{}-{}.json",
            std::process::id(),
            name
        ));
        let path = path.to_str().unwrap().to_string();
        write_keypair_file(&keypair, &path).unwrap();
        (keypair, path)
    }

    #[test]
    fn load_signer_reads_keypair_file() {
        let (keypair, path) = temp_keypair_file("load-signer");
        let signer = load_signer(&path, "--payer").unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(signer.pubkey(), keypair.pubkey());
    }

    #[test]
    fn load_signer_reports_missing_file_with_flag() {
        let err = load_signer("/nonexistent/payer.json", "--payer")
            .err()
            .unwrap();
        let msg = err.to_string();
        assert!(msg.contains("--payer"), "got: {msg}");
        assert!(msg.contains("/nonexistent/payer.json"), "got: {msg}");
    }

    #[test]
    fn load_signer_rejects_malformed_kms_uri() {
        // Fails at URI parsing, before any KMS client or network access.
        let err = load_signer("kms://not-a-resource-name", "--payer")
            .err()
            .unwrap();
        assert!(err.to_string().contains("pubkey="), "got: {err}");
    }

    #[test]
    fn read_keypair_file_checked_rejects_kms_uri() {
        let err = read_keypair_file_checked(
            "kms://projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1?pubkey=4zvwRjXUKGfvwnParsHAS3HuSVzV5cA4McphgmoCtajS",
            "--payer",
        )
        .unwrap_err();
        assert!(err.to_string().contains("not supported"), "got: {err}");
    }
}
