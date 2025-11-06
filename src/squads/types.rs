//! Squads v4 type definitions and helpers
//!
//! Type definitions extracted from squads-multisig-program source.
//! All types, constants, and helper functions for PDA derivation and discriminators.

use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;

// ============================================================================
// Constants
// ============================================================================

/// Squads v4 program ID on SphereNet
pub const SQUADS_PROGRAM_ID: &str = "SqdsYSe3QC9aGdU5p8y7Y3HtT5VrVLEtYCNjN37yBTh";

/// System program ID
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

// PDA seeds (from squads-multisig-program/src/state/seeds.rs)
pub const SEED_PREFIX: &[u8] = b"multisig";
pub const SEED_MULTISIG: &[u8] = b"multisig";
pub const SEED_PROGRAM_CONFIG: &[u8] = b"program_config";
pub const SEED_TRANSACTION: &[u8] = b"transaction";
pub const SEED_PROPOSAL: &[u8] = b"proposal";

// ============================================================================
// Type Definitions (from squads-multisig-program/src/state/multisig.rs)
// ============================================================================

/// Multisig member with permissions
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Member {
    pub key: Pubkey,
    pub permissions: Permissions,
}

/// Permission bitmask
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Permissions {
    pub mask: u8,
}

impl Permissions {
    /// All permissions: Initiate | Vote | Execute
    pub fn all() -> Self {
        Self {
            mask: Permission::Initiate as u8 | Permission::Vote as u8 | Permission::Execute as u8,
        }
    }
}

/// Individual permission flags
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Permission {
    Initiate = 1 << 0, // Can create proposals
    Vote = 1 << 1,     // Can vote on proposals
    Execute = 1 << 2,  // Can execute approved proposals
}

/// Arguments for program_config_init instruction
#[derive(BorshSerialize, BorshDeserialize)]
pub struct ProgramConfigInitArgs {
    /// The authority that can configure the program config
    pub authority: Pubkey,
    /// The fee charged for creating a new multisig (in lamports)
    pub multisig_creation_fee: u64,
    /// The treasury where the creation fee is transferred to
    pub treasury: Pubkey,
}

/// Arguments for multisig_create_v2 instruction
#[derive(BorshSerialize, BorshDeserialize)]
pub struct MultisigCreateArgsV2 {
    pub config_authority: Option<Pubkey>,
    pub threshold: u16,
    pub members: Vec<Member>,
    pub time_lock: u32,
    pub rent_collector: Option<Pubkey>,
    pub memo: Option<String>,
}

/// Arguments for vault_transaction_create instruction
#[derive(BorshSerialize, BorshDeserialize)]
pub struct VaultTransactionCreateArgs {
    /// Index of the vault this transaction belongs to
    pub vault_index: u8,
    /// Number of ephemeral signing PDAs required by the transaction
    pub ephemeral_signers: u8,
    /// Serialized transaction message (borsh-serialized TransactionMessage)
    pub transaction_message: Vec<u8>,
    /// Optional memo for the transaction
    pub memo: Option<String>,
}

/// Arguments for proposal_create instruction
#[derive(BorshSerialize, BorshDeserialize)]
pub struct ProposalCreateArgs {
    /// Index of the multisig transaction this proposal is associated with
    pub transaction_index: u64,
    /// Whether the proposal should be initialized with status `Draft`
    pub draft: bool,
}

// ============================================================================
// Helpers
// ============================================================================

/// Calculate Anchor instruction discriminator
///
/// Discriminator = sha256("namespace:instruction_name")[0..8]
pub fn anchor_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[0..8]);
    disc
}

/// Derive program config PDA
///
/// seeds: [b"multisig", b"program_config"]
pub fn get_program_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_PROGRAM_CONFIG], program_id)
}

/// Derive multisig PDA from create_key
///
/// seeds: [b"multisig", b"multisig", create_key]
pub fn get_multisig_pda(create_key: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[SEED_PREFIX, SEED_MULTISIG, create_key.as_ref()],
        program_id,
    )
}

/// Derive vault transaction PDA
///
/// seeds: [b"multisig", multisig, b"transaction", transaction_index]
pub fn get_vault_transaction_pda(
    multisig: &Pubkey,
    transaction_index: u64,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_PREFIX,
            multisig.as_ref(),
            SEED_TRANSACTION,
            &transaction_index.to_le_bytes(),
        ],
        program_id,
    )
}

/// Derive proposal PDA
///
/// seeds: [b"multisig", multisig, b"transaction", transaction_index, b"proposal"]
pub fn get_proposal_pda(
    multisig: &Pubkey,
    transaction_index: u64,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_PREFIX,
            multisig.as_ref(),
            SEED_TRANSACTION,
            &transaction_index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        program_id,
    )
}
