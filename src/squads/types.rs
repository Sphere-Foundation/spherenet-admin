//! Squads v4 type definitions and helpers
//!
//! Type definitions extracted from squads-multisig-program source.
//! All types, constants, and helper functions for PDA derivation and discriminators.

use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use std::io::{Read as IoRead, Write};
use std::marker::PhantomData;

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
pub const SEED_VAULT: &[u8] = b"vault";

// ============================================================================
// SmallVec (from squads-multisig-program/src/utils/small_vec.rs)
// ============================================================================

/// Concise serialization schema for vectors where the length can be represented
/// by any type `L` (typically unsigned integer like `u8` or `u16`)
#[derive(Clone, Debug, Default)]
pub struct SmallVec<L, T>(Vec<T>, PhantomData<L>);

impl<L, T> From<SmallVec<L, T>> for Vec<T> {
    fn from(val: SmallVec<L, T>) -> Self {
        val.0
    }
}

impl<L, T> From<Vec<T>> for SmallVec<L, T> {
    fn from(val: Vec<T>) -> Self {
        Self(val, PhantomData)
    }
}

impl<T: BorshSerialize> BorshSerialize for SmallVec<u8, T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let len = u8::try_from(self.0.len()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        writer.write_all(&len.to_le_bytes())?;
        for item in &self.0 {
            item.serialize(writer)?;
        }
        Ok(())
    }
}

impl<T: BorshDeserialize> BorshDeserialize for SmallVec<u8, T> {
    fn deserialize_reader<R: IoRead>(reader: &mut R) -> std::io::Result<Self> {
        let mut len_bytes = [0u8; 1];
        reader.read_exact(&mut len_bytes)?;
        let len = u8::from_le_bytes(len_bytes) as usize;

        let mut vec = Vec::with_capacity(len);
        for _ in 0..len {
            vec.push(T::deserialize_reader(reader)?);
        }
        Ok(Self(vec, PhantomData))
    }
}

impl<T: BorshSerialize> BorshSerialize for SmallVec<u16, T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let len = u16::try_from(self.0.len()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        writer.write_all(&len.to_le_bytes())?;
        for item in &self.0 {
            item.serialize(writer)?;
        }
        Ok(())
    }
}

impl<T: BorshDeserialize> BorshDeserialize for SmallVec<u16, T> {
    fn deserialize_reader<R: IoRead>(reader: &mut R) -> std::io::Result<Self> {
        let mut len_bytes = [0u8; 2];
        reader.read_exact(&mut len_bytes)?;
        let len = u16::from_le_bytes(len_bytes) as usize;

        let mut vec = Vec::with_capacity(len);
        for _ in 0..len {
            vec.push(T::deserialize_reader(reader)?);
        }
        Ok(Self(vec, PhantomData))
    }
}

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

/// Multisig account structure (from squads-multisig-program/src/state/multisig.rs)
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Multisig {
    /// Key that is used to seed the multisig PDA
    pub create_key: Pubkey,
    /// The authority that can change the multisig config
    /// Set to Pubkey::default() for autonomous multisig
    pub config_authority: Pubkey,
    /// Threshold for signatures
    pub threshold: u16,
    /// How many seconds must pass between transaction voting settlement and execution
    pub time_lock: u32,
    /// Last transaction index (0 means no transactions created)
    pub transaction_index: u64,
    /// Last stale transaction index
    pub stale_transaction_index: u64,
    /// Rent reclamation address (None disables rent reclamation)
    pub rent_collector: Option<Pubkey>,
    /// Bump for the multisig PDA seed
    pub bump: u8,
    /// Members of the multisig
    pub members: Vec<Member>,
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

/// Squads TransactionMessage format (for vault_transaction_create)
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct TransactionMessage {
    pub num_signers: u8,
    pub num_writable_signers: u8,
    pub num_writable_non_signers: u8,
    pub account_keys: SmallVec<u8, Pubkey>,
    pub instructions: SmallVec<u8, CompiledInstruction>,
    pub address_table_lookups: SmallVec<u8, MessageAddressTableLookup>,
}

/// VaultTransaction account structure (simplified for reading)
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VaultTransaction {
    pub multisig: Pubkey,
    pub creator: Pubkey,
    pub index: u64,
    pub bump: u8,
    pub vault_index: u8,
    pub vault_bump: u8,
    pub ephemeral_signer_bumps: Vec<u8>,
    pub message: VaultTransactionMessage,
}

/// VaultTransactionMessage (stored in VaultTransaction)
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VaultTransactionMessage {
    pub num_signers: u8,
    pub num_writable_signers: u8,
    pub num_writable_non_signers: u8,
    pub account_keys: Vec<Pubkey>,
    pub instructions: Vec<MultisigCompiledInstruction>,
    pub address_table_lookups: Vec<MultisigMessageAddressTableLookup>,
}

/// Compiled instruction in VaultTransactionMessage
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MultisigCompiledInstruction {
    pub program_id_index: u8,
    pub account_indexes: Vec<u8>,
    pub data: Vec<u8>,
}

/// Address table lookup in VaultTransactionMessage
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MultisigMessageAddressTableLookup {
    pub account_key: Pubkey,
    pub writable_indexes: Vec<u8>,
    pub readonly_indexes: Vec<u8>,
}

/// Compiled instruction for TransactionMessage
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    pub account_indexes: SmallVec<u8, u8>,
    pub data: SmallVec<u16, u8>,
}

/// Address table lookup for TransactionMessage
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MessageAddressTableLookup {
    pub account_key: Pubkey,
    pub writable_indexes: Vec<u8>,
    pub readonly_indexes: Vec<u8>,
}

/// Parse transaction_index from a multisig account's data
///
/// Multisig layout:
/// - discriminator(8)
/// - create_key(32)
/// - config_authority(32)
/// - threshold(2)
/// - time_lock(4)
/// - transaction_index(8)  <- at offset 78
/// - ...
pub fn parse_transaction_index(account_data: &[u8]) -> eyre::Result<u64> {
    const OFFSET: usize = 8 + 32 + 32 + 2 + 4; // 78
    if account_data.len() < OFFSET + 8 {
        return Err(eyre::eyre!("Invalid multisig account data (too short)"));
    }
    let tx_index_bytes: [u8; 8] = account_data[OFFSET..OFFSET + 8]
        .try_into()
        .map_err(|_| eyre::eyre!("Failed to parse transaction_index"))?;
    Ok(u64::from_le_bytes(tx_index_bytes))
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
/// seeds: [b"multisig", multisig, b"transaction", transaction_index + 1]
///
/// Note: The program uses `multisig.transaction_index.checked_add(1)` for the PDA,
/// so we need to add 1 to the transaction_index when deriving the PDA.
pub fn get_vault_transaction_pda(
    multisig: &Pubkey,
    transaction_index: u64,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let next_index = transaction_index
        .checked_add(1)
        .expect("transaction_index overflow");
    Pubkey::find_program_address(
        &[
            SEED_PREFIX,
            multisig.as_ref(),
            SEED_TRANSACTION,
            &next_index.to_le_bytes(),
        ],
        program_id,
    )
}

/// Derive proposal PDA
///
/// seeds: [b"multisig", multisig, b"transaction", transaction_index + 1, b"proposal"]
///
/// Note: Like vault_transaction, proposals reference the NEXT transaction index.
pub fn get_proposal_pda(
    multisig: &Pubkey,
    transaction_index: u64,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let next_index = transaction_index
        .checked_add(1)
        .expect("transaction_index overflow");
    Pubkey::find_program_address(
        &[
            SEED_PREFIX,
            multisig.as_ref(),
            SEED_TRANSACTION,
            &next_index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        program_id,
    )
}

/// Derive vault PDA from multisig
///
/// seeds: [b"multisig", multisig, b"vault", vault_index]
pub fn get_vault_pda(multisig: &Pubkey, vault_index: u8, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_PREFIX,
            multisig.as_ref(),
            SEED_VAULT,
            &vault_index.to_le_bytes(),
        ],
        program_id,
    )
}

/// Compile a Solana instruction into Squads TransactionMessage format
///
/// This builds the TransactionMessage with the vault as the fee payer/signer.
pub fn compile_instruction_to_transaction_message(
    instruction: &Instruction,
    vault_pubkey: &Pubkey,
) -> TransactionMessage {
    // Collect all unique account keys
    let mut account_keys = Vec::new();
    let mut account_key_indexes = std::collections::HashMap::new();

    // Add vault as first account (writable signer)
    account_keys.push(*vault_pubkey);
    account_key_indexes.insert(*vault_pubkey, 0u8);

    // Add all accounts from the instruction
    for account_meta in &instruction.accounts {
        if !account_key_indexes.contains_key(&account_meta.pubkey) {
            let index = account_keys.len() as u8;
            account_keys.push(account_meta.pubkey);
            account_key_indexes.insert(account_meta.pubkey, index);
        }
    }

    // Add program ID if not already in account_keys
    if !account_key_indexes.contains_key(&instruction.program_id) {
        let index = account_keys.len() as u8;
        account_keys.push(instruction.program_id);
        account_key_indexes.insert(instruction.program_id, index);
    }

    // Reorder accounts: writable signers, readonly signers, writable non-signers, readonly non-signers
    let mut writable_signers = vec![*vault_pubkey]; // Vault is always writable signer
    let mut readonly_signers = Vec::new();
    let mut writable_non_signers = Vec::new();
    let mut readonly_non_signers = Vec::new();

    for account_meta in &instruction.accounts {
        if account_meta.pubkey == *vault_pubkey {
            continue; // Already added as first account
        }
        if account_meta.is_signer {
            if account_meta.is_writable {
                writable_signers.push(account_meta.pubkey);
            } else {
                readonly_signers.push(account_meta.pubkey);
            }
        } else {
            if account_meta.is_writable {
                writable_non_signers.push(account_meta.pubkey);
            } else {
                readonly_non_signers.push(account_meta.pubkey);
            }
        }
    }

    // Add program ID to readonly non-signers if not already there
    if instruction.program_id != *vault_pubkey
        && !readonly_non_signers.contains(&instruction.program_id)
    {
        readonly_non_signers.push(instruction.program_id);
    }

    // Build final ordered account_keys
    let mut ordered_keys = Vec::new();
    ordered_keys.extend(&writable_signers);
    ordered_keys.extend(&readonly_signers);
    ordered_keys.extend(&writable_non_signers);
    ordered_keys.extend(&readonly_non_signers);

    // Build index map for ordered keys
    let mut key_index_map = std::collections::HashMap::new();
    for (i, key) in ordered_keys.iter().enumerate() {
        key_index_map.insert(*key, i as u8);
    }

    // Compile the instruction
    let program_id_index = *key_index_map.get(&instruction.program_id).unwrap();
    let account_indexes: Vec<u8> = instruction
        .accounts
        .iter()
        .map(|meta| *key_index_map.get(&meta.pubkey).unwrap())
        .collect();

    let compiled_instruction = CompiledInstruction {
        program_id_index,
        account_indexes: account_indexes.into(),
        data: instruction.data.clone().into(),
    };

    let num_signers = (writable_signers.len() + readonly_signers.len()) as u8;
    let num_writable_signers = writable_signers.len() as u8;
    let num_writable_non_signers = writable_non_signers.len() as u8;

    TransactionMessage {
        num_signers,
        num_writable_signers,
        num_writable_non_signers,
        account_keys: ordered_keys.into(),
        instructions: vec![compiled_instruction].into(),
        address_table_lookups: Vec::new().into(), // No lookups for simple transactions
    }
}
