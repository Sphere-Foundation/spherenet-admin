//! BLS keypair derivation for vote accounts (Alpenglow / SIMD-0464).
//!
//! SphereNet vote accounts carry a BLS pubkey derived deterministically from the
//! validator identity, plus a proof of possession bound to the vote account. This
//! mirrors what genesis and the `spherenet` client's `create-vote-account` do, so
//! that `spherenet-admin vote create` produces identical, consensus-ready accounts.
//!
//! Two values are lifted verbatim from the client (`agave-votor-messages` and
//! `solana-vote-program`), neither of which is published as a standalone crate:
//! * the derivation seed `b"alpenglow"`, and
//! * the proof-of-possession message layout (`b"ALPENGLOW"` ‖ vote_pubkey ‖ bls_pubkey).
//!
//! The on-chain program verifies the PoP against exactly this message, so the
//! label and field order must match byte-for-byte.

use solana_bls_signatures::{
    keypair::Keypair as BlsKeypair, PubkeyCompressed, BLS_PROOF_OF_POSSESSION_COMPRESSED_SIZE,
    BLS_PUBLIC_KEY_COMPRESSED_SIZE,
};
use solana_sdk::{pubkey::Pubkey, signature::Signer};

/// Seed used to derive the BLS keypair from the identity signer.
/// (`agave_votor_messages::consensus_message::BLS_KEYPAIR_DERIVE_SEED`)
const BLS_KEYPAIR_DERIVE_SEED: &[u8] = b"alpenglow";

/// Domain label prefixing the proof-of-possession message.
/// (`solana_vote_program::vote_state::generate_pop_message`)
const POP_LABEL: &[u8; 9] = b"ALPENGLOW";

const POP_MESSAGE_SIZE: usize =
    POP_LABEL.len() + std::mem::size_of::<Pubkey>() + BLS_PUBLIC_KEY_COMPRESSED_SIZE;

/// A BLS keypair derived from a validator identity: the compressed pubkey and
/// proof of possession that [`VoteInitV2`] needs, plus a base64 `display` of the
/// compressed pubkey — the exact value stored on-chain in the vote account.
/// (Note: `solana-keygen bls_pubkey` / genesis render the *affine* form of the
/// same key; the underlying key is identical.)
///
/// [`VoteInitV2`]: solana_vote_interface::state::VoteInitV2
pub struct DerivedBls {
    pub pubkey: [u8; BLS_PUBLIC_KEY_COMPRESSED_SIZE],
    pub proof_of_possession: [u8; BLS_PROOF_OF_POSSESSION_COMPRESSED_SIZE],
    pub display: String,
}

/// Derive the identity's BLS pubkey and a proof of possession bound to
/// `vote_account`. The derivation has the identity *sign* the seed, so the BLS
/// key is a deterministic function of the identity keypair — same identity,
/// same BLS key, every time (verified: matches `solana-keygen bls_pubkey`).
pub fn derive_pubkey_and_pop(
    identity: &dyn Signer,
    vote_account: &Pubkey,
) -> eyre::Result<DerivedBls> {
    let bls_keypair = BlsKeypair::derive_from_signer(identity, BLS_KEYPAIR_DERIVE_SEED)
        .map_err(|e| eyre::eyre!("Failed to derive BLS keypair from identity: {e}"))?;

    let pubkey = bls_keypair.public.to_bytes_compressed();
    let message = generate_pop_message(vote_account, &pubkey);
    let proof_of_possession = bls_keypair
        .proof_of_possession(Some(&message))
        .to_bytes_compressed();

    // Base64 of the compressed pubkey — the on-chain representation.
    let display = PubkeyCompressed(pubkey).to_string();

    Ok(DerivedBls {
        pubkey,
        proof_of_possession,
        display,
    })
}

/// `b"ALPENGLOW"` ‖ vote_account_pubkey ‖ compressed_bls_pubkey — byte-for-byte
/// identical to `solana_vote_program`'s `generate_pop_message`.
fn generate_pop_message(
    vote_account: &Pubkey,
    bls_pubkey: &[u8; BLS_PUBLIC_KEY_COMPRESSED_SIZE],
) -> [u8; POP_MESSAGE_SIZE] {
    let mut message = [0u8; POP_MESSAGE_SIZE];
    let (label, rest) = message.split_at_mut(POP_LABEL.len());
    let (pubkey, bls) = rest.split_at_mut(std::mem::size_of::<Pubkey>());
    label.copy_from_slice(POP_LABEL);
    pubkey.copy_from_slice(vote_account.as_ref());
    bls.copy_from_slice(bls_pubkey);
    message
}
