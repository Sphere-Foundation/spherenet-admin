//! Multisig write commands: create a vault, approve and execute proposals.

use crate::cli::output::{emit, progress, subfield, OutputMode, Render, TxOutputView};
use crate::squads::{self, Member, Permissions};
use borsh::BorshDeserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

/// Result of creating a multisig vault — the create-key reference plus the
/// derived PDAs and the creation signature.
#[derive(serde::Serialize)]
pub struct CreatedMultisigView {
    create_key: String,
    multisig: String,
    vault: String,
    signature: String,
}

impl Render for CreatedMultisigView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Multisig vault created\n");
        out.push_str(&subfield("Create Key", &self.create_key));
        out.push_str(&subfield("Multisig PDA", &self.multisig));
        out.push_str(&subfield("Vault PDA", &self.vault));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

/// Create a new multisig vault
///
/// # Arguments
/// * `members_str` - Comma-separated list of member pubkeys
/// * `threshold` - Number of approvals required
/// * `create_key_path` - Path to keypair for PDA derivation
/// * `payer_path` - Path to payer keypair
/// * `url` - RPC URL
/// * `time_lock` - Optional time delay (seconds)
/// * `memo` - Optional description
pub fn create(
    members_str: String,
    threshold: u16,
    create_key_path: String,
    payer_path: String,
    url: &str,
    time_lock: Option<u32>,
    memo: Option<String>,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress("Creating Squads v4 multisig vault...");

    // Parse member pubkeys
    let member_pubkeys: Vec<Pubkey> = members_str
        .split(',')
        .map(|s| {
            let trimmed = s.trim();
            Pubkey::from_str(trimmed)
                .map_err(|e| eyre::eyre!("Invalid pubkey '{}': {}", trimmed, e))
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    if member_pubkeys.is_empty() {
        eyre::bail!("At least one member is required");
    }

    // Validate threshold
    if threshold == 0 {
        eyre::bail!("Threshold must be at least 1");
    }
    if threshold as usize > member_pubkeys.len() {
        eyre::bail!(
            "Threshold ({}) cannot exceed number of members ({})",
            threshold,
            member_pubkeys.len()
        );
    }

    // Echo what we're creating (progress → stderr)
    progress(format!("Members: {}", member_pubkeys.len()));
    for (i, pk) in member_pubkeys.iter().enumerate() {
        progress(format!("  [{}] {}", i + 1, pk));
    }
    progress(format!("Threshold: {}/{}", threshold, member_pubkeys.len()));
    if let Some(tl) = time_lock {
        if tl > 0 {
            progress(format!("Time Lock: {} seconds", tl));
        }
    }
    if let Some(ref m) = memo {
        progress(format!("Memo: {}", m));
    }

    // Load keypairs
    let create_key = solana_sdk::signature::read_keypair_file(&create_key_path)
        .map_err(|e| eyre::eyre!("Failed to read create key '{}': {}", create_key_path, e))?;

    let payer = solana_sdk::signature::read_keypair_file(&payer_path)
        .map_err(|e| eyre::eyre!("Failed to read payer key '{}': {}", payer_path, e))?;

    // Create RPC client
    let rpc = RpcClient::new(url);

    // Parse program ID
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // Derive PDAs
    let (multisig_pda, _) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);
    let (program_config_pda, _) = squads::types::get_program_config_pda(&program_id);

    // Fetch program config to get treasury
    let program_config_account = rpc.get_account(&program_config_pda)
        .map_err(|e| eyre::eyre!("Failed to fetch program_config account at {}: {}. Is the Squads ProgramConfig baked into genesis?", program_config_pda, e))?;

    // Parse treasury from account data
    // ProgramConfig layout: discriminator(8) + authority(32) + multisig_creation_fee(8) + treasury(32)
    if program_config_account.data.len() < 80 {
        eyre::bail!("Invalid program_config account data");
    }
    let treasury_bytes: [u8; 32] = program_config_account.data[48..80].try_into()?;
    let treasury = Pubkey::new_from_array(treasury_bytes);

    // Build members with full permissions
    let members: Vec<Member> = member_pubkeys
        .iter()
        .map(|key| Member {
            key: *key,
            permissions: Permissions::all(),
        })
        .collect();

    // Build args
    let args = squads::types::MultisigCreateArgsV2 {
        config_authority: None, // Immutable multisig
        threshold,
        members,
        time_lock: time_lock.unwrap_or(0),
        rent_collector: None, // Defaults to creator
        memo,
    };

    // Build instruction
    let create_ix = squads::ixs::build_multisig_create_v2_ix(
        &program_id,
        &program_config_pda,
        &treasury, // Must match program_config.treasury
        &multisig_pda,
        &create_key.pubkey(),
        &payer.pubkey(),
        args,
    )?;

    progress("Sending transaction...");

    // Send transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&payer.pubkey()),
        &[&payer, &create_key], // Both must sign
        recent_blockhash,
    );
    let signature = rpc.send_and_confirm_transaction(&tx)?;

    let (vault_pda, _) = squads::types::get_vault_pda(&multisig_pda, 0, &program_id);
    emit(
        &CreatedMultisigView {
            create_key: create_key.pubkey().to_string(),
            multisig: multisig_pda.to_string(),
            vault: vault_pda.to_string(),
            signature: signature.to_string(),
        },
        mode,
    )
}

/// Wrap an arbitrary instruction into a Squads proposal.
///
/// Builds `vault_transaction_create` + `proposal_create` in one transaction
/// signed by `member`, targeting the multisig's default vault (index 0). This is
/// the multisig path behind `Authority::execute_instruction` — governance ops
/// (mp/vw/pw) reach it via that adapter; it can also be called directly.
pub fn propose(
    rpc: &RpcClient,
    multisig: &Pubkey,
    member: &Keypair,
    instruction: Instruction,
    description: &str,
) -> eyre::Result<TxOutputView> {
    progress(format!("Creating proposal: {}", description));

    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // Fetch multisig account to get the next transaction index
    let multisig_account = rpc
        .get_account(multisig)
        .map_err(|e| eyre::eyre!("Failed to fetch multisig account at {}: {}", multisig, e))?;
    let current_transaction_index = squads::types::parse_transaction_index(&multisig_account.data)?;
    let next_transaction_index = current_transaction_index
        .checked_add(1)
        .ok_or_else(|| eyre::eyre!("Transaction index overflow"))?;

    progress(format!("Transaction index: {}", next_transaction_index));

    // Derive PDAs
    let (vault_transaction_pda, _) =
        squads::types::get_vault_transaction_pda(multisig, current_transaction_index, &program_id);
    let (proposal_pda, _) =
        squads::types::get_proposal_pda(multisig, current_transaction_index, &program_id);
    // Default vault (index 0) — the account that signs the wrapped instruction.
    let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);

    // Compile the instruction into a Squads TransactionMessage (signed by the vault PDA)
    let transaction_message =
        squads::types::compile_instruction_to_transaction_message(&instruction, &vault_pda);
    let transaction_message_bytes = borsh::to_vec(&transaction_message)
        .map_err(|e| eyre::eyre!("Failed to serialize transaction message: {}", e))?;

    let vault_tx_args = squads::types::VaultTransactionCreateArgs {
        vault_index: 0,
        ephemeral_signers: 0,
        transaction_message: transaction_message_bytes,
        memo: Some(description.to_string()),
    };
    let vault_tx_create_ix = squads::ixs::build_vault_transaction_create_ix(
        &program_id,
        multisig,
        &vault_transaction_pda,
        &member.pubkey(),
        &member.pubkey(), // rent_payer
        vault_tx_args,
    )?;

    let proposal_args = squads::types::ProposalCreateArgs {
        transaction_index: next_transaction_index,
        draft: false, // Active proposal (ready for voting)
    };
    let proposal_create_ix = squads::ixs::build_proposal_create_ix(
        &program_id,
        multisig,
        &proposal_pda,
        &member.pubkey(),
        &member.pubkey(), // rent_payer
        proposal_args,
    )?;

    // Send both instructions in one transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[vault_tx_create_ix, proposal_create_ix],
        Some(&member.pubkey()),
        &[member],
        recent_blockhash,
    );
    let signature = rpc.send_and_confirm_transaction(&tx)?;

    Ok(TxOutputView::ProposalCreated {
        proposal: proposal_pda.to_string(),
        transaction_index: next_transaction_index,
        signature: signature.to_string(),
    })
}

/// Approve a multisig proposal
///
/// # Arguments
/// * `create_key` - The multisig's create-key (pubkey)
/// * `transaction_index` - Transaction index of the proposal to approve
/// * `member_path` - Path to the member keypair who is approving
/// * `url` - RPC URL
pub fn approve(
    create_key: String,
    transaction_index: u64,
    member_path: String,
    url: &str,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress(format!("Approving proposal (tx index {})...", transaction_index));

    let create_key = create_key
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid create-key '{}': {}", create_key, e))?;

    // Load member key
    let member = solana_sdk::signature::read_keypair_file(&member_path)
        .map_err(|e| eyre::eyre!("Failed to read member key: {}", e))?;

    let rpc = RpcClient::new(url);

    // Resolve + validate the multisig from its create-key
    let multisig_pda = squads::resolve(&rpc, &create_key)?.multisig;

    // Derive proposal PDA
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
    let (proposal_pda, _) =
        squads::types::get_proposal_pda(&multisig_pda, transaction_index - 1, &program_id);

    progress(format!("Multisig: {}", multisig_pda));
    progress(format!("Proposal: {}", proposal_pda));
    progress(format!("Member:   {}", member.pubkey()));

    // Build approve instruction
    let approve_ix = squads::ixs::build_proposal_approve_ix(
        &program_id,
        &multisig_pda,
        &proposal_pda,
        &member.pubkey(),
    )?;

    // Send transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[approve_ix],
        Some(&member.pubkey()),
        &[&member],
        recent_blockhash,
    );

    progress("Sending approval transaction...");
    let signature = rpc.send_and_confirm_transaction(&tx)?;

    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}

/// Execute an approved multisig proposal
///
/// # Arguments
/// * `create_key` - The multisig's create-key (pubkey)
/// * `transaction_index` - Transaction index of the proposal to execute
/// * `member_path` - Path to the member keypair who is executing
/// * `url` - RPC URL
pub fn execute(
    create_key: String,
    transaction_index: u64,
    member_path: String,
    url: &str,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress(format!("Executing proposal (tx index {})...", transaction_index));

    let create_key = create_key
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid create-key '{}': {}", create_key, e))?;

    // Load member key
    let member = solana_sdk::signature::read_keypair_file(&member_path)
        .map_err(|e| eyre::eyre!("Failed to read member key: {}", e))?;

    let rpc = RpcClient::new(url);

    // Resolve + validate the multisig from its create-key
    let multisig_pda = squads::resolve(&rpc, &create_key)?.multisig;

    // Derive PDAs
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
    let (proposal_pda, _) =
        squads::types::get_proposal_pda(&multisig_pda, transaction_index - 1, &program_id);
    let (vault_transaction_pda, _) =
        squads::types::get_vault_transaction_pda(&multisig_pda, transaction_index - 1, &program_id);

    // Fetch and parse the vault transaction to get the exact accounts
    let vault_tx_account = rpc
        .get_account(&vault_transaction_pda)
        .map_err(|e| eyre::eyre!("Failed to fetch vault transaction: {}", e))?;

    // Deserialize VaultTransaction (skip 8-byte Anchor discriminator)
    if vault_tx_account.data.len() < 8 {
        eyre::bail!("Invalid vault transaction account data");
    }
    let data_slice = &vault_tx_account.data[8..];
    let vault_tx = squads::types::VaultTransaction::deserialize(&mut &data_slice[..])
        .map_err(|e| eyre::eyre!("Failed to deserialize vault transaction: {}", e))?;

    // Derive vault PDA using stored bump (what the program uses for signing)
    let stored_vault_pda = Pubkey::create_program_address(
        &[
            squads::types::SEED_PREFIX,
            multisig_pda.as_ref(),
            squads::types::SEED_VAULT,
            &vault_tx.vault_index.to_le_bytes(),
            &[vault_tx.vault_bump],
        ],
        &program_id,
    )
    .map_err(|e| eyre::eyre!("Failed to derive vault PDA with stored bump: {}", e))?;

    // Build remaining_accounts from the message's account_keys
    // We need to determine which are writable/readonly based on the message headers
    let num_writable_signers = vault_tx.message.num_writable_signers as usize;
    let num_readonly_signers =
        (vault_tx.message.num_signers - vault_tx.message.num_writable_signers) as usize;
    let num_writable_non_signers = vault_tx.message.num_writable_non_signers as usize;

    // Determine if each account is a signer based on the message headers
    // PDAs (vault and ephemeral signers) cannot be marked as signers
    let is_signer_index = |index: usize| -> bool { index < vault_tx.message.num_signers as usize };

    let mut remaining_accounts = Vec::new();
    for (i, key) in vault_tx.message.account_keys.iter().enumerate() {
        let is_writable = if i < num_writable_signers {
            true // Writable signer
        } else if i < num_writable_signers + num_readonly_signers {
            false // Readonly signer
        } else if i < num_writable_signers + num_readonly_signers + num_writable_non_signers {
            true // Writable non-signer
        } else {
            false // Readonly non-signer
        };

        // From Squads SDK: PDAs (vault_pda and ephemeral signers) cannot be marked as signers
        let is_signer = is_signer_index(i) && key != &stored_vault_pda;

        remaining_accounts.push(AccountMeta {
            pubkey: *key,
            is_signer,
            is_writable,
        });
    }

    // Build execute instruction
    let execute_ix = squads::ixs::build_vault_transaction_execute_ix(
        &program_id,
        &multisig_pda,
        &proposal_pda,
        &vault_transaction_pda,
        &member.pubkey(),
        remaining_accounts,
    )?;

    // Send transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[execute_ix],
        Some(&member.pubkey()),
        &[&member],
        recent_blockhash,
    );

    let signature = rpc.send_and_confirm_transaction(&tx)?;

    emit(
        &TxOutputView::Executed {
            signature: signature.to_string(),
        },
        mode,
    )
}
