//! BLS keypair derivation for vote accounts (Alpenglow / SIMD-0464).
//!
//! SphereNet vote accounts use the `VoteStateV4` layout, which carries a BLS
//! pubkey derived deterministically from the validator identity plus a proof of
//! possession bound to the vote account. Deriving it here lets `vote create`
//! produce the same V4-layout accounts that genesis bakes into the bootstrap
//! validators — a layout-compatibility choice, not a consensus requirement.
//!
//! Two values are lifted verbatim from the client (`agave-votor-messages` and
//! `solana-vote-program`), neither of which is published as a standalone crate:
//! * the derivation seed `b"alpenglow"`, and
//! * the proof-of-possession message layout (`b"ALPENGLOW"` ‖ vote_pubkey).
//!
//! The on-chain program verifies the PoP against exactly this message
//! (`solana_vote_program::vote_state::generate_pop_message`, "ALPENGLOW" ‖ the
//! 32-byte vote pubkey = 41 bytes — the BLS pubkey is NOT part of the message),
//! so the label and field order must match byte-for-byte.

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

const POP_MESSAGE_SIZE: usize = POP_LABEL.len() + std::mem::size_of::<Pubkey>();

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
    let message = generate_pop_message(vote_account);
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

/// `b"ALPENGLOW"` ‖ vote_account_pubkey (41 bytes) — byte-for-byte identical to
/// `solana_vote_program`'s `generate_pop_message`. The compressed BLS pubkey is
/// deliberately NOT included: the on-chain `verify_bls_proof_of_possession`
/// signs/verifies this exact 41-byte message, so appending the pubkey (as an
/// earlier version did) produces a PoP the chain rejects.
fn generate_pop_message(vote_account: &Pubkey) -> [u8; POP_MESSAGE_SIZE] {
    let mut message = [0u8; POP_MESSAGE_SIZE];
    let (label, pubkey) = message.split_at_mut(POP_LABEL.len());
    label.copy_from_slice(POP_LABEL);
    pubkey.copy_from_slice(vote_account.as_ref());
    message
}
