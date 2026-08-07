//! CLI signer resolution.
//!
//! Loads a signer from a keypair-file path or a `kms://` URI (a key held in
//! GCP Cloud KMS — see [`crate::authority::kms`]), plus the shared signer-dedup
//! helper used across command modules.

use crate::authority::kms;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signature},
    signer::{Signer, SignerError},
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

/// A resolved CLI signer: a local keypair file, or a key held in GCP Cloud KMS.
///
/// A concrete, `Debug`-able alternative to `Box<dyn Signer>` for the fixed set
/// of signer sources the admin CLI supports (which is what lets
/// [`crate::authority::Authority`] derive `Debug`). [`AdminSigner::as_ref`]
/// yields the `&dyn Signer` the Solana signing APIs expect, so call sites read
/// the same as they did with the boxed form.
pub enum AdminSigner {
    /// A keypair loaded from a local file.
    File(Keypair),
    /// A key held in GCP Cloud KMS.
    Kms(kms::KmsSigner),
}

impl AdminSigner {
    /// Borrow as a `&dyn Signer` for the Solana signing APIs (drop-in for
    /// `Box::<dyn Signer>::as_ref`).
    pub fn as_ref(&self) -> &dyn Signer {
        match self {
            AdminSigner::File(k) => k,
            AdminSigner::Kms(k) => k,
        }
    }
}

impl Signer for AdminSigner {
    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        self.as_ref().try_pubkey()
    }
    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        self.as_ref().try_sign_message(message)
    }
    fn is_interactive(&self) -> bool {
        self.as_ref().is_interactive()
    }
}

impl std::fmt::Debug for AdminSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            AdminSigner::File(_) => "File",
            AdminSigner::Kms(_) => "Kms",
        };
        f.debug_tuple(kind).field(&self.pubkey()).finish()
    }
}

/// Load a signer from a CLI value: a keypair file path, or a `kms://` URI for
/// a key held in GCP Cloud KMS (see [`crate::authority::kms`]).
///
/// This is the default loader for every keypair argument; `flag` names the
/// argument for error messages. Arguments that must stay file-based use
/// [`read_keypair_file_checked`] instead.
pub fn load_signer(path: &str, flag: &str) -> eyre::Result<AdminSigner> {
    if path.starts_with(kms::KMS_URI_SCHEME) {
        Ok(AdminSigner::Kms(kms::KmsSigner::from_uri(path)?))
    } else {
        let keypair = read_keypair_file(path)
            .map_err(|e| eyre::eyre!("Failed to read keypair for {flag} from {path}: {e}"))?;
        Ok(AdminSigner::File(keypair))
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
