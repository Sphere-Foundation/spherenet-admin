//! CLI signer resolution.
//!
//! Loads a signer from a keypair-file path or a `kms://` URI (a key held in
//! GCP Cloud KMS — see [`crate::authority::kms`]), plus the shared signer-dedup
//! helper used across command modules.

use crate::authority::kms;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
};

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
/// a key held in GCP Cloud KMS (see [`crate::authority::kms`]).
///
/// This is the default loader for every keypair argument; `flag` names the
/// argument for error messages. Arguments that must stay file-based use
/// [`read_keypair_file_checked`] instead.
pub fn load_signer(path: &str, flag: &str) -> eyre::Result<Box<dyn Signer>> {
    if path.starts_with(kms::KMS_URI_SCHEME) {
        Ok(Box::new(kms::KmsSigner::from_uri(path)?))
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
    if path.starts_with(kms::KMS_URI_SCHEME) {
        eyre::bail!("KMS signers are not supported for {flag}; pass a keypair file path");
    }
    read_keypair_file(path)
        .map_err(|e| eyre::eyre!("Failed to read keypair for {flag} from {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::write_keypair_file;

    fn temp_keypair_file(name: &str) -> (Keypair, String) {
        let keypair = Keypair::new();
        let path = std::env::temp_dir().join(format!(
            "spherenet-admin-signer-test-{}-{}.json",
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
