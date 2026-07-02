use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spherenet_authority::Authority;
use spherenet_program_whitelist_client::instructions::{AddEntryBuilder, RemoveEntryBuilder};
use spherenet_program_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ProgramWhitelistAccount, load, whitelist_entry::ProgramWhitelistEntry},
};
use std::sync::LazyLock;

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(Pubkey::default);

#[derive(serde::Serialize)]
struct ProgramWhitelistView {
    program_id: String,
    account: String,
    authority: String,
    pending_authority: String,
    deployers: Vec<String>,
}

impl Render for ProgramWhitelistView {
    fn to_text(&self) -> String {
        let mut out = boxed_header("Program Whitelist Account");
        out.push_str(newline());
        out.push_str(&field("Program ID", &self.program_id));
        out.push_str(&field("Whitelist Account", &self.account));
        out.push_str(newline());
        out.push_str(&field("Authority", &self.authority));
        out.push_str(&field("Pending Authority", &self.pending_authority));
        out.push_str(newline());
        out.push_str(&format!(
            "Whitelisted Deployer Authorities ({}):",
            self.deployers.len()
        ));
        out.push_str(newline());
        if self.deployers.is_empty() {
            out.push_str("  (none)");
        } else {
            for d in &self.deployers {
                out.push_str(&subfield("Deployer Authority", d));
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
    let whitelist = load::<ProgramWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize program whitelist account: {:?}", e))?;

    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let mut deployers = Vec::new();
    for (pubkey, account) in rpc_client.get_program_accounts(&program_id)? {
        if pubkey == whitelist_pubkey {
            continue; // skip the main whitelist account itself
        }
        if let Ok(entry) = load::<ProgramWhitelistEntry>(&account.data) {
            deployers.push(Pubkey::from(entry.entry_address).to_string());
        }
    }

    let view = ProgramWhitelistView {
        program_id: program_id.to_string(),
        account: whitelist_pubkey.to_string(),
        authority: Pubkey::from(whitelist.authority).to_string(),
        pending_authority: Pubkey::from(whitelist.pending_authority).to_string(),
        deployers,
    };
    emit(&view, mode)
}

/// Derives the program whitelist entry PDA for a deployer authority.
///
/// The PDA is derived using:
/// - Seeds: `[program_whitelist_account_id, deployer_authority]`
/// - Program: program whitelist program ID
///
/// # Arguments
/// * `deployer_authority` - Pubkey of the authority who can deploy/upgrade programs
///
/// # Returns
/// * `(Pubkey, u8)` - The derived PDA and bump seed
pub fn derive_whitelist_entry(deployer_authority: &Pubkey) -> (Pubkey, u8) {
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    Pubkey::find_program_address(
        &[whitelist_pubkey.as_ref(), deployer_authority.as_ref()],
        &program_id,
    )
}

pub fn add(rpc_url: &str, program_authority: String, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse deployer authority (who can deploy/upgrade programs)
    let deployer_pubkey = program_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deployer authority: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deployer_pubkey);

    println!("\nWhitelisting deployer authority:");
    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("  Deployer Authority:  {}", deployer_pubkey);
    println!("  Whitelist Entry PDA: {}", whitelist_entry_pda);
    println!("  Whitelist Authority: {}", instruction_authority);

    // Build the instruction
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction = AddEntryBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(instruction_authority)
        .whitelist_entry_account(whitelist_entry_pda)
        .payer(instruction_authority)
        .system_program(*SYSTEM_PROGRAM)
        .program_authority(deployer_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Whitelist deployer authority {}", deployer_pubkey);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn remove(rpc_url: &str, program_authority: String, authority: Authority) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Parse deployer authority (who can deploy/upgrade programs)
    let deployer_pubkey = program_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deployer authority: {}", e))?;

    // Derive the whitelist entry PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deployer_pubkey);

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nRemoving deployer authority from whitelist:");
    println!("  Deployer Authority:  {}", deployer_pubkey);
    println!("  Whitelist Entry PDA: {}", whitelist_entry_pda);
    println!("  Whitelist Authority: {}", instruction_authority);

    // Build the instruction
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction = RemoveEntryBuilder::new()
        .whitelist_account(whitelist_pubkey)
        .whitelist_authority(instruction_authority)
        .whitelist_entry_account(whitelist_entry_pda)
        .destination_account(instruction_authority) // Reclaim lamports to authority
        .system_program(*SYSTEM_PROGRAM)
        .program_authority(deployer_pubkey)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Remove deployer authority {} from whitelist",
        deployer_pubkey
    );
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

/// Requires that the upgrade authority is whitelisted, returning the whitelist entry PDA.
///
/// This function performs a "fail-fast" check before spending any lamports by:
/// 1. Deriving the whitelist PDA from the upgrade authority pubkey
/// 2. Querying the RPC to verify the whitelist entry account exists
/// 3. Returning the PDA for use in deploy/upgrade instructions (account index 8 for deploy, 7 for upgrade)
///
/// # Arguments
/// * `rpc_client` - RPC client for querying account state
/// * `upgrade_authority` - Pubkey of the program's upgrade authority (must be whitelisted)
///
/// # Returns
/// * `Ok(Pubkey)` - The whitelist entry PDA if authority is whitelisted
/// * `Err` - If authority is not whitelisted, with instructions to add them
pub fn require_whitelist_entry(
    rpc_client: &RpcClient,
    upgrade_authority: Pubkey,
) -> eyre::Result<Pubkey> {
    println!("\n🔐 Verifying whitelist...");

    // Derive whitelist PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&upgrade_authority);

    println!("  Whitelist entry PDA: {}", whitelist_entry_pda);

    // Verify whitelist entry account exists on-chain
    match rpc_client.get_account(&whitelist_entry_pda) {
        Ok(_) => {
            println!("  ✓ Upgrade authority is whitelisted");
            Ok(whitelist_entry_pda)
        }
        Err(_) => {
            Err(eyre::eyre!(
                "❌ Upgrade authority {} is not whitelisted!\n   Run: spherenet-admin pw add {} --auth <AUTHORITY>",
                upgrade_authority,
                upgrade_authority
            ))
        }
    }
}
