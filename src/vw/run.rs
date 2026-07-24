use crate::cli::authority::Authority;
use crate::cli::output::{emit, progress, OutputMode};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::read_keypair_file;
use solana_sdk::signer::Signer;
use spherenet_validator_whitelist_client::instructions::{
    ApproveWhitelistEntryBuilder, RejectWhitelistEntryBuilder, RemoveWhitelistEntryBuilder,
    RequestWhitelistEntryBuilder, UpdateEndEpochBuilder, UpdateStartEpochBuilder,
};
use spherenet_validator_whitelist_interface::{account_solana, program_solana};
use std::sync::LazyLock;

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(Pubkey::default);

/// Derives the validator whitelist entry PDA for a vote account.
///
/// The PDA is derived using:
/// - Seeds: `[vote_account]`
/// - Program: validator whitelist program ID
///
/// # Arguments
/// * `vote_account` - Pubkey of the validator's vote account
///
/// # Returns
/// * `(Pubkey, u8)` - The derived PDA and bump seed
pub fn derive_whitelist_entry(vote_account: &Pubkey) -> (Pubkey, u8) {
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    Pubkey::find_program_address(&[vote_account.as_ref()], &program_id)
}

/// Step 1 of the two-step flow: **request** a whitelist entry.
///
/// Creates a `Pending` entry that the whitelist authority must later `approve`.
/// The validator's vote account must **co-sign** to prove control of it, so this
/// is single-sig only (the `authority`/payer keypair signs + pays; the vote
/// account keypair co-signs). A multisig cannot carry the vote-account signature.
pub fn request(
    rpc_url: &str,
    vote_account_keypair: String,
    start_epoch: Option<u64>,
    end_epoch: Option<u64>,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // The vote account co-signs to prove control — load its keypair.
    let vote_account = read_keypair_file(&vote_account_keypair).map_err(|e| {
        eyre::eyre!(
            "Failed to load vote account keypair from {}: {}",
            vote_account_keypair,
            e
        )
    })?;
    let vote_account_pubkey = vote_account.pubkey();

    // Get current epoch if start_epoch not provided
    let current_epoch = rpc_client.get_epoch_info()?.epoch;
    let start_epoch = start_epoch.unwrap_or(current_epoch + 1);
    let end_epoch = end_epoch.unwrap_or(u64::MAX);

    // Validate epochs
    if start_epoch >= end_epoch {
        return Err(eyre::eyre!("Start epoch must be less than end epoch"));
    }

    if start_epoch < current_epoch {
        return Err(eyre::eyre!(
            "Start epoch {} is in the past (current epoch: {})",
            start_epoch,
            current_epoch
        ));
    }

    // Derive the whitelist entry PDA to be created
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let payer = authority.instruction_authority_pubkey()?;

    progress("Requesting validator whitelist entry:");
    progress(format!("Vote Account:    {}", vote_account_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry_pda));
    progress(format!("Start Epoch:     {}", start_epoch));
    progress(format!(
        "End Epoch:       {}",
        if end_epoch == u64::MAX {
            "∞".to_string()
        } else {
            end_epoch.to_string()
        }
    ));
    progress(format!("Payer:           {}", payer));

    let instruction = RequestWhitelistEntryBuilder::new()
        .payer(payer)
        .validator_vote_account(vote_account_pubkey)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .system_program(*SYSTEM_PROGRAM)
        .start_epoch(start_epoch.to_le_bytes())
        .end_epoch(end_epoch.to_le_bytes())
        .instruction();

    let description = format!(
        "Request whitelist entry for validator {}",
        vote_account_pubkey
    );
    let result = authority.execute_instruction_with_cosigners(
        &rpc_client,
        instruction,
        &[&vote_account],
        &description,
    )?;
    emit(&result, mode)
}

/// Step 2 of the two-step flow: **approve** a pending whitelist entry.
///
/// Authority action — moves the entry from `Pending` to `Approved` and sets its
/// active term (`start_epoch`/`end_epoch`). Works single-sig or via multisig.
pub fn approve(
    rpc_url: &str,
    vote_account: String,
    start_epoch: Option<u64>,
    end_epoch: Option<u64>,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Get current epoch if start_epoch not provided
    let current_epoch = rpc_client.get_epoch_info()?.epoch;
    let start_epoch = start_epoch.unwrap_or(current_epoch + 1);
    let end_epoch = end_epoch.unwrap_or(u64::MAX);

    if start_epoch >= end_epoch {
        return Err(eyre::eyre!("Start epoch must be less than end epoch"));
    }

    if start_epoch < current_epoch {
        return Err(eyre::eyre!(
            "Start epoch {} is in the past (current epoch: {})",
            start_epoch,
            current_epoch
        ));
    }

    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Approving validator whitelist entry:");
    progress(format!("Vote Account:    {}", vote_account_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry_pda));
    progress(format!("Start Epoch:     {}", start_epoch));
    progress(format!(
        "End Epoch:       {}",
        if end_epoch == u64::MAX {
            "∞".to_string()
        } else {
            end_epoch.to_string()
        }
    ));
    progress(format!("Authority:       {}", instruction_authority));

    let instruction = ApproveWhitelistEntryBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .vote_account_pubkey(vote_account_pubkey)
        .start_epoch(start_epoch.to_le_bytes())
        .end_epoch(end_epoch.to_le_bytes())
        .instruction();

    let description = format!(
        "Approve whitelist entry for validator {}",
        vote_account_pubkey
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

/// Reject a pending whitelist entry (authority action). Closes the `Pending`
/// entry without approving it. Works single-sig or via multisig.
pub fn reject(
    rpc_url: &str,
    vote_account: String,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Rejecting validator whitelist entry:");
    progress(format!("Vote Account:    {}", vote_account_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry_pda));
    progress(format!("Authority:       {}", instruction_authority));

    let instruction = RejectWhitelistEntryBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    let description = format!(
        "Reject whitelist entry for validator {}",
        vote_account_pubkey
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn remove(
    rpc_url: &str,
    vote_account: String,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Removing validator from whitelist:");
    progress(format!("Vote Account:    {}", vote_account_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry_pda));
    progress(format!("Authority:       {}", instruction_authority));

    // Build the instruction
    let instruction = RemoveWhitelistEntryBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    let description = format!("Remove validator {} from whitelist", vote_account_pubkey);
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn update_start_epoch(
    rpc_url: &str,
    vote_account: String,
    epoch: u64,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Updating validator start epoch:");
    progress(format!("Vote Account:    {}", vote_account_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry_pda));
    progress(format!("New Start Epoch: {}", epoch));
    progress(format!("Authority:       {}", instruction_authority));

    // Build the instruction
    let instruction = UpdateStartEpochBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .new_start_epoch(epoch.to_le_bytes())
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    let description = format!(
        "Update validator {} start epoch to {}",
        vote_account_pubkey, epoch
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn update_end_epoch(
    rpc_url: &str,
    vote_account: String,
    epoch: u64,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    let epoch_display = if epoch == u64::MAX {
        "∞".to_string()
    } else {
        epoch.to_string()
    };

    progress("Updating validator end epoch:");
    progress(format!("Vote Account:    {}", vote_account_pubkey));
    progress(format!("Whitelist Entry: {}", whitelist_entry_pda));
    progress(format!("New End Epoch:   {}", epoch_display));
    progress(format!("Authority:       {}", instruction_authority));

    // Build the instruction
    let instruction = UpdateEndEpochBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .new_end_epoch(epoch.to_le_bytes())
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    let description = format!(
        "Update validator {} end epoch to {}",
        vote_account_pubkey, epoch_display
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}
