//! Signer helpers shared across commands that build multi-signer transactions.

use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

/// Deduplicate signers by pubkey, preserving order (first occurrence wins).
///
/// Several roles (funder, fee payer, authority, new account) frequently
/// collapse onto one keypair. A transaction that lists the same key twice as a
/// signer is rejected, so callers pass every role and let this drop duplicates.
pub fn dedupe<'a>(signers: &[&'a Keypair]) -> Vec<&'a Keypair> {
    let mut seen: Vec<Pubkey> = Vec::with_capacity(signers.len());
    let mut out: Vec<&Keypair> = Vec::with_capacity(signers.len());
    for kp in signers {
        let pk = kp.pubkey();
        if !seen.contains(&pk) {
            seen.push(pk);
            out.push(kp);
        }
    }
    out
}
