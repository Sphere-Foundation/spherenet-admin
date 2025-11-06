//! Squads v4 instruction builders
//!
//! Manual instruction building for Squads v4 program.
//! Each function builds a specific instruction type.

use super::types::*;
use borsh::BorshSerialize;
use eyre::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Build program_config_init instruction
///
/// One-time initialization of the program config PDA.
/// Must be signed by the hardcoded INITIALIZER key in the program.
pub fn build_program_config_init_ix(
    program_id: &Pubkey,
    program_config: &Pubkey,
    initializer: &Pubkey,
    args: ProgramConfigInitArgs,
) -> Result<Instruction> {
    // Anchor discriminator for "program_config_init"
    let discriminator = anchor_discriminator("global", "program_config_init");

    // Serialize: [discriminator (8 bytes)] + [borsh-serialized args]
    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    args.serialize(&mut data)?;

    // System program ID
    let system_program = SYSTEM_PROGRAM_ID.parse::<Pubkey>()?;

    // Accounts (order from ProgramConfigInit in instructions/program_config_init.rs)
    let accounts = vec![
        AccountMeta::new(*program_config, false), // program_config (init)
        AccountMeta::new(*initializer, true),     // initializer (signer)
        AccountMeta::new_readonly(system_program, false), // system_program
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Build multisig_create_v2 instruction
///
/// Creates a new multisig vault with specified members and threshold.
pub fn build_multisig_create_v2_ix(
    program_id: &Pubkey,
    program_config: &Pubkey,
    treasury: &Pubkey,
    multisig: &Pubkey,
    create_key: &Pubkey,
    creator: &Pubkey,
    args: MultisigCreateArgsV2,
) -> Result<Instruction> {
    // Anchor discriminator for "multisig_create_v2"
    let discriminator = anchor_discriminator("global", "multisig_create_v2");

    // Serialize: [discriminator (8 bytes)] + [borsh-serialized args]
    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    args.serialize(&mut data)?;

    // System program ID
    let system_program = SYSTEM_PROGRAM_ID.parse::<Pubkey>()?;

    // Accounts (order from MultisigCreateV2 in instructions/multisig_create.rs)
    let accounts = vec![
        AccountMeta::new_readonly(*program_config, false), // program_config
        AccountMeta::new(*treasury, false),                // treasury
        AccountMeta::new(*multisig, false),                // multisig
        AccountMeta::new_readonly(*create_key, true),      // create_key (signer)
        AccountMeta::new(*creator, true),                  // creator (signer)
        AccountMeta::new_readonly(system_program, false),  // system_program
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Build vault_transaction_create instruction
///
/// Creates a new vault transaction in the multisig.
/// The transaction_index will be multisig.transaction_index + 1.
pub fn build_vault_transaction_create_ix(
    program_id: &Pubkey,
    multisig: &Pubkey,
    transaction: &Pubkey,
    creator: &Pubkey,
    rent_payer: &Pubkey,
    args: VaultTransactionCreateArgs,
) -> Result<Instruction> {
    // Anchor discriminator for "vault_transaction_create"
    let discriminator = anchor_discriminator("global", "vault_transaction_create");

    // Serialize: [discriminator (8 bytes)] + [borsh-serialized args]
    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    args.serialize(&mut data)?;

    // System program ID
    let system_program = SYSTEM_PROGRAM_ID.parse::<Pubkey>()?;

    // Accounts (order from VaultTransactionCreate in instructions/vault_transaction_create.rs)
    let accounts = vec![
        AccountMeta::new(*multisig, false),               // multisig (mut)
        AccountMeta::new(*transaction, false),            // transaction (init, mut)
        AccountMeta::new_readonly(*creator, true),        // creator (signer)
        AccountMeta::new(*rent_payer, true),              // rent_payer (mut, signer)
        AccountMeta::new_readonly(system_program, false), // system_program
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Build proposal_create instruction
///
/// Creates a new proposal for an existing vault transaction.
pub fn build_proposal_create_ix(
    program_id: &Pubkey,
    multisig: &Pubkey,
    proposal: &Pubkey,
    creator: &Pubkey,
    rent_payer: &Pubkey,
    args: ProposalCreateArgs,
) -> Result<Instruction> {
    // Anchor discriminator for "proposal_create"
    let discriminator = anchor_discriminator("global", "proposal_create");

    // Serialize: [discriminator (8 bytes)] + [borsh-serialized args]
    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    args.serialize(&mut data)?;

    // System program ID
    let system_program = SYSTEM_PROGRAM_ID.parse::<Pubkey>()?;

    // Accounts (order from ProposalCreate in instructions/proposal_create.rs)
    let accounts = vec![
        AccountMeta::new_readonly(*multisig, false), // multisig (readonly)
        AccountMeta::new(*proposal, false),          // proposal (init, mut)
        AccountMeta::new_readonly(*creator, true),   // creator (signer)
        AccountMeta::new(*rent_payer, true),         // rent_payer (mut, signer)
        AccountMeta::new_readonly(system_program, false), // system_program
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

// TODO: Add more instruction builders as needed:
// - build_proposal_approve_ix
// - build_vault_transaction_execute_ix
// - etc.
