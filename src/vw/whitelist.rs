use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spherenet_authority::Authority;
use spherenet_validator_whitelist_client::instructions::{
    AddToWhitelistBuilder, RemoveFromWhitelistBuilder, UpdateEndEpochBuilder,
    UpdateStartEpochBuilder,
};
use spherenet_validator_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ValidatorWhitelistAccount, load, whitelist_entry::ValidatorWhitelistEntry},
};
use std::sync::LazyLock;

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(Pubkey::default);

#[derive(serde::Serialize)]
struct WhitelistEntryView {
    vote_account: String,
    start_epoch: u64,
    /// `None` = no end (on-chain sentinel `u64::MAX`, shown as ∞).
    end_epoch: Option<u64>,
}

#[derive(serde::Serialize)]
struct WhitelistView {
    program_id: String,
    account: String,
    authority: String,
    pending_authority: String,
    validator_count: u32,
    validators: Vec<WhitelistEntryView>,
}

impl Render for WhitelistView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Validator Whitelist Account");
        out.push_str(newline());
        out.push_str(&field("Program ID", &self.program_id));
        out.push_str(&field("Whitelist Account", &self.account));
        out.push_str(newline());
        out.push_str(&field("Authority", &self.authority));
        out.push_str(&field("Pending Authority", &self.pending_authority));
        out.push_str(newline());
        out.push_str(&format!(
            "Whitelisted Validators ({}):",
            self.validator_count
        ));
        out.push_str(newline());
        if self.validators.is_empty() {
            out.push_str("  (none)");
        } else {
            for e in &self.validators {
                out.push_str(newline());
                out.push_str(&subfield("Vote Account", &e.vote_account));
                out.push_str(&subfield("Start Epoch", e.start_epoch));
                out.push_str(&subfield(
                    "End Epoch",
                    e.end_epoch
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "∞".to_string()),
                ));
            }
        }
        out
    }
}

pub fn show(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ValidatorWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize whitelist account: {:?}", e))?;

    let validator_count = u32::from_le_bytes(whitelist.validator_amount);

    // Only scan program accounts when there's something to find.
    let mut validators = Vec::new();
    if validator_count > 0 {
        let program_id = Pubkey::from(program_solana::id().to_bytes());
        for (pubkey, account) in rpc_client.get_program_accounts(&program_id)? {
            if pubkey == whitelist_pubkey {
                continue; // skip the main whitelist account itself
            }
            if let Ok(entry) = load::<ValidatorWhitelistEntry>(&account.data) {
                let end_epoch = u64::from_le_bytes(entry.end_epoch);
                validators.push(WhitelistEntryView {
                    vote_account: Pubkey::from(entry.pubkey).to_string(),
                    start_epoch: u64::from_le_bytes(entry.start_epoch),
                    end_epoch: (end_epoch != u64::MAX).then_some(end_epoch),
                });
            }
        }
    }

    let view = WhitelistView {
        program_id: Pubkey::from(program_solana::id().to_bytes()).to_string(),
        account: whitelist_pubkey.to_string(),
        authority: Pubkey::from(whitelist.authority).to_string(),
        pending_authority: Pubkey::from(whitelist.pending_authority).to_string(),
        validator_count,
        validators,
    };
    emit(&view, mode)
}

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

pub fn add(
    rpc_url: &str,
    vote_account: String,
    start_epoch: Option<u64>,
    end_epoch: Option<u64>,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

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

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nAdding validator to whitelist:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!("  Start Epoch:     {}", start_epoch);
    println!(
        "  End Epoch:       {}",
        if end_epoch == u64::MAX {
            "∞".to_string()
        } else {
            end_epoch.to_string()
        }
    );
    let instruction_authority = authority.instruction_authority_pubkey()?;
    println!("  Authority:       {}", instruction_authority);

    // Build the instruction
    let instruction = AddToWhitelistBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .validator_vote_account(vote_account_pubkey)
        .system_program(*SYSTEM_PROGRAM)
        .start_epoch(start_epoch.to_le_bytes())
        .end_epoch(end_epoch.to_le_bytes())
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Add validator {} to whitelist", vote_account_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn remove(rpc_url: &str, vote_account: String, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nRemoving validator from whitelist:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!("  Authority:       {}", instruction_authority);

    // Build the instruction
    let instruction = RemoveFromWhitelistBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Remove validator {} from whitelist", vote_account_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn update_start_epoch(
    rpc_url: &str,
    vote_account: String,
    epoch: u64,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nUpdating validator start epoch:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!("  New Start Epoch: {}", epoch);
    println!("  Authority:       {}", instruction_authority);

    // Build the instruction
    let instruction = UpdateStartEpochBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .new_start_epoch(epoch.to_le_bytes())
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Update validator {} start epoch to {}",
        vote_account_pubkey, epoch
    );
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn update_end_epoch(
    rpc_url: &str,
    vote_account: String,
    epoch: u64,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse vote account pubkey
    let vote_account_pubkey = vote_account
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid vote account pubkey: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&vote_account_pubkey);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nUpdating validator end epoch:");
    println!("  Vote Account:    {}", vote_account_pubkey);
    println!("  Whitelist Entry: {}", whitelist_entry_pda);
    println!(
        "  New End Epoch:   {}",
        if epoch == u64::MAX {
            "∞".to_string()
        } else {
            epoch.to_string()
        }
    );

    let instruction_authority = authority.instruction_authority_pubkey()?;
    println!("  Authority:       {}", instruction_authority);

    // Build the instruction
    let instruction = UpdateEndEpochBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .validator_whitelist(whitelist_pubkey)
        .whitelist_entry(whitelist_entry_pda)
        .new_end_epoch(epoch.to_le_bytes())
        .vote_account_pubkey(vote_account_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Update validator {} end epoch to {}",
        vote_account_pubkey,
        if epoch == u64::MAX {
            "∞".to_string()
        } else {
            epoch.to_string()
        }
    );
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}
