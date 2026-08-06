use crate::cli::authority::Authority;
use crate::cli::output::{emit, progress, OutputMode};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use spherenet_program_whitelist_client::instructions::{
    ApproveWhitelistEntryBuilder, RejectWhitelistEntryBuilder, RemoveWhitelistEntryBuilder,
    RequestWhitelistEntryBuilder,
};
use spherenet_program_whitelist_interface::{
    account_solana, program_solana,
    state::{load, whitelist_entry::ProgramWhitelistEntry, EntryState},
};
use std::sync::LazyLock;

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(Pubkey::default);

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

/// Step 1 of the two-step flow: **request** a deployer whitelist entry.
///
/// Creates a `Pending` entry the whitelist authority must later `approve`. The
/// deploy authority must **co-sign** to prove control of the key, so this is
/// single-sig only (the `authority`/payer keypair signs + pays; the deploy
/// authority keypair co-signs). A multisig cannot carry the co-signature.
pub fn request(
    rpc_url: &str,
    deploy_authority_keypair: String,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // The deploy authority co-signs to prove control — load its keypair.
    let deploy_authority = crate::utils::run::load_signer(
        &deploy_authority_keypair,
        "the deploy-authority co-signer",
    )?;
    let deploy_authority_pubkey = deploy_authority.pubkey();

    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deploy_authority_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let payer = authority.instruction_authority_pubkey()?;

    progress("Requesting program whitelist entry:");
    progress(format!("Deploy Authority:    {}", deploy_authority_pubkey));
    progress(format!("Whitelist Entry PDA: {}", whitelist_entry_pda));
    progress(format!("Payer:               {}", payer));

    let instruction = RequestWhitelistEntryBuilder::new()
        .payer(payer)
        .deploy_authority(deploy_authority_pubkey)
        .whitelist_account(whitelist_pubkey)
        .whitelist_entry_account(whitelist_entry_pda)
        .system_program(*SYSTEM_PROGRAM)
        .instruction();

    let description = format!(
        "Request program whitelist entry for {}",
        deploy_authority_pubkey
    );
    let result = authority.execute_instruction_with_cosigners(
        &rpc_client,
        instruction,
        &[deploy_authority.as_ref()],
        &description,
    )?;
    emit(&result, mode)
}

/// Step 2 of the two-step flow: **approve** a pending deployer whitelist entry
/// (authority action — moves it from `Pending` to `Approved`). Single-sig or
/// multisig.
pub fn approve(
    rpc_url: &str,
    deploy_authority: String,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let deploy_authority_pubkey = deploy_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deploy authority: {}", e))?;

    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deploy_authority_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Approving deployer whitelist entry:");
    progress(format!("Deploy Authority:    {}", deploy_authority_pubkey));
    progress(format!("Whitelist Entry PDA: {}", whitelist_entry_pda));
    progress(format!("Whitelist Authority: {}", instruction_authority));

    let instruction = ApproveWhitelistEntryBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .whitelist_account(whitelist_pubkey)
        .whitelist_entry_account(whitelist_entry_pda)
        .deploy_authority(deploy_authority_pubkey)
        .instruction();

    let description = format!(
        "Approve deployer whitelist entry for {}",
        deploy_authority_pubkey
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

/// Reject a pending deployer whitelist entry (authority action). Single-sig or
/// multisig.
pub fn reject(
    rpc_url: &str,
    deploy_authority: String,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let deploy_authority_pubkey = deploy_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deploy authority: {}", e))?;

    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deploy_authority_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Rejecting deployer whitelist entry:");
    progress(format!("Deploy Authority:    {}", deploy_authority_pubkey));
    progress(format!("Whitelist Entry PDA: {}", whitelist_entry_pda));
    progress(format!("Whitelist Authority: {}", instruction_authority));

    let instruction = RejectWhitelistEntryBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .whitelist_account(whitelist_pubkey)
        .whitelist_entry_account(whitelist_entry_pda)
        .deploy_authority(deploy_authority_pubkey)
        .instruction();

    let description = format!(
        "Reject deployer whitelist entry for {}",
        deploy_authority_pubkey
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

/// Remove a deployer whitelist entry (authority action; works on `Pending` or
/// `Approved`). Reclaimed rent goes to the payer. Single-sig or multisig.
pub fn remove(
    rpc_url: &str,
    deploy_authority: String,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let deploy_authority_pubkey = deploy_authority
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid deploy authority: {}", e))?;

    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&deploy_authority_pubkey);
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Removing deployer authority from whitelist:");
    progress(format!("Deploy Authority:    {}", deploy_authority_pubkey));
    progress(format!("Whitelist Entry PDA: {}", whitelist_entry_pda));
    progress(format!("Whitelist Authority: {}", instruction_authority));

    let instruction = RemoveWhitelistEntryBuilder::new()
        .payer(instruction_authority)
        .whitelist_authority(instruction_authority)
        .whitelist_account(whitelist_pubkey)
        .whitelist_entry_account(whitelist_entry_pda)
        .deploy_authority(deploy_authority_pubkey)
        .instruction();

    let description = format!(
        "Remove deployer authority {} from whitelist",
        deploy_authority_pubkey
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
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
    progress("🔐 Verifying whitelist...");

    // Derive whitelist PDA
    let (whitelist_entry_pda, _bump) = derive_whitelist_entry(&upgrade_authority);

    progress(format!("Whitelist entry PDA: {}", whitelist_entry_pda));

    // Verify the whitelist entry exists AND is Approved. Under the two-step flow
    // a `Pending` entry also exists on-chain (created at request time) but is NOT
    // valid for deployment — the loader gates on Approved, so fail-fast here too.
    match rpc_client.get_account(&whitelist_entry_pda) {
        Ok(account) => {
            let entry = load::<ProgramWhitelistEntry>(&account.data).map_err(|e| {
                eyre::eyre!(
                    "Failed to read whitelist entry {}: {:?}",
                    whitelist_entry_pda,
                    e
                )
            })?;
            if entry.state == EntryState::Approved as u8 {
                progress("✅ Upgrade authority is whitelisted (approved)");
                Ok(whitelist_entry_pda)
            } else {
                Err(eyre::eyre!(
                    "❌ Upgrade authority {} has a PENDING (not yet approved) whitelist entry.\n   An authority must approve it first:\n     spherenet-admin pw approve {} --auth <AUTHORITY>",
                    upgrade_authority,
                    upgrade_authority
                ))
            }
        }
        Err(_) => Err(eyre::eyre!(
            "❌ Upgrade authority {} is not whitelisted!\n   The deployer requests, then an authority approves:\n     spherenet-admin pw request <DEPLOY_AUTHORITY_KEYPAIR> --auth <PAYER>\n     spherenet-admin pw approve {} --auth <AUTHORITY>",
            upgrade_authority,
            upgrade_authority
        )),
    }
}
