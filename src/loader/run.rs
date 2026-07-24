//! Program deployment operations: deploy, upgrade, and extend.

use crate::cli::authority::Authority;
use crate::cli::output::{emit, progress, subfield, OutputMode, Render};
use crate::pw::run::require_whitelist_entry;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::bpf_loader_upgradeable;
#[allow(deprecated)]
use spherenet_loader_v3_interface::{
    instruction::{
        create_buffer, deploy_with_max_program_len, extend_program as extend_program_ix,
        set_buffer_authority, set_upgrade_authority as set_upgrade_authority_ix, upgrade, write,
    },
    state::UpgradeableLoaderState,
};
use std::{fs, str::FromStr};

/// Conservative chunk size for writing program data to avoid transaction size limits
const MAX_WRITE_SIZE: usize = 900;

/// Result of a `deploy` — the program address and its upgrade authority.
#[derive(serde::Serialize)]
pub struct DeployedProgramView {
    program_id: String,
    upgrade_authority: String,
    signature: String,
}

impl Render for DeployedProgramView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Program deployed\n");
        out.push_str(&subfield("Program ID", &self.program_id));
        out.push_str(&subfield("Upgrade Authority", &self.upgrade_authority));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

/// Writes program data to a buffer account in chunks.
///
/// Program data is split into MAX_WRITE_SIZE chunks (900 bytes) to avoid hitting
/// transaction size limits. Each chunk is written via a separate transaction signed
/// by both the payer (who pays transaction fees) and the authority (buffer owner).
fn write_buffer(
    rpc_client: &RpcClient,
    payer: &Keypair,
    authority_keypair: &Keypair,
    buffer: &Pubkey,
    authority: &Pubkey,
    program_data: &[u8],
) -> eyre::Result<()> {
    let chunks: Vec<_> = program_data.chunks(MAX_WRITE_SIZE).collect();
    let total_chunks = chunks.len();

    for (i, chunk) in chunks.into_iter().enumerate() {
        let offset = i * MAX_WRITE_SIZE;

        let write_ix = write(buffer, authority, offset as u32, chunk.to_vec());

        let mut transaction = Transaction::new_with_payer(&[write_ix], Some(&payer.pubkey()));
        transaction.sign(
            &[payer, authority_keypair],
            rpc_client.get_latest_blockhash()?,
        );
        rpc_client.send_and_confirm_transaction(&transaction)?;

        progress(format!(
            "Writing chunk {}/{} ({} bytes) ✅",
            i + 1,
            total_chunks,
            chunk.len()
        ));
    }

    progress("✅ Program data written successfully");
    Ok(())
}

/// Deploy a program to SphereNet
pub fn deploy(
    url: &str,
    program_so_path: String,
    program_keypair_path: String,
    upgrade_authority_path: String,
    payer_keypair_path: String,
    max_data_len: Option<usize>,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress("🚀 Deploying program to SphereNet...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Load keypairs
    let payer = read_keypair_file(&payer_keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read payer keypair from {}: {}",
            payer_keypair_path,
            e
        )
    })?;
    let program_keypair = read_keypair_file(&program_keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read program keypair from {}: {}",
            program_keypair_path,
            e
        )
    })?;
    let program_id = program_keypair.pubkey();
    let upgrade_authority_keypair = read_keypair_file(&upgrade_authority_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read upgrade authority keypair from {}: {}",
            upgrade_authority_path,
            e
        )
    })?;
    let upgrade_authority = upgrade_authority_keypair.pubkey();

    progress(format!("Program ID: {}", program_id));
    progress(format!("Payer: {}", payer.pubkey()));
    progress(format!("Upgrade Authority: {}", upgrade_authority));

    // Read program .so file
    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre::eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    progress(format!("Program size: {} bytes", program_data.len()));

    // Determine max data length
    let max_data_len_provided = max_data_len.is_some();
    let max_data_len = max_data_len.unwrap_or(program_data.len());
    if max_data_len < program_data.len() {
        return Err(eyre::eyre!(
            "max_data_len ({}) must be at least program size ({})",
            max_data_len,
            program_data.len()
        ));
    }

    // Warn if no max-data-len specified (important for multisig scenarios)
    if !max_data_len_provided {
        progress("⚠️  WARNING: No --max-data-len specified, using program size as capacity.");
        progress(
            "   If you plan to transfer upgrade authority to multisig, you CANNOT extend later!",
        );
        progress(
            "   Consider deploying with generous --max-data-len (e.g., --max-data-len 500000)",
        );
    }

    // Verify upgrade authority is whitelisted before spending lamports (fail-fast)
    let whitelist_entry = require_whitelist_entry(&rpc_client, upgrade_authority)?;

    // Create and write buffer
    progress("📝 Creating buffer account...");
    let buffer_keypair = Keypair::new();
    let buffer_pubkey = buffer_keypair.pubkey();
    let buffer_size = UpgradeableLoaderState::size_of_buffer(program_data.len());
    let buffer_lamports = rpc_client.get_minimum_balance_for_rent_exemption(buffer_size)?;

    progress(format!("Buffer size: {} bytes", buffer_size));
    progress(format!("Buffer rent: {} lamports", buffer_lamports));

    // Create and initialize buffer account with UPGRADE AUTHORITY as authority
    // (required for deployment - buffer authority must match program's upgrade authority)
    let create_buffer_instructions = create_buffer(
        &payer.pubkey(),
        &buffer_pubkey,
        &upgrade_authority, // Must match the program's upgrade authority
        buffer_lamports,
        program_data.len(),
    )?;

    let mut transaction =
        Transaction::new_with_payer(&create_buffer_instructions, Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &buffer_keypair],
        rpc_client.get_latest_blockhash()?,
    );
    rpc_client.send_and_confirm_transaction(&transaction)?;

    progress(format!("✅ Buffer account created: {}", buffer_pubkey));

    // Write program data to buffer in chunks (upgrade authority signs as buffer authority)
    progress("📤 Writing program data to buffer...");
    write_buffer(
        &rpc_client,
        &payer,
        &upgrade_authority_keypair,
        &buffer_pubkey,
        &upgrade_authority,
        &program_data,
    )?;

    // Deploy program with whitelist validation
    progress("🎯 Deploying program...");
    let program_lamports = rpc_client
        .get_minimum_balance_for_rent_exemption(UpgradeableLoaderState::size_of_program())?;

    let deploy_instructions = deploy_with_max_program_len(
        &payer.pubkey(),
        &program_id,
        &buffer_pubkey,
        &upgrade_authority,
        program_lamports,
        max_data_len,
        &whitelist_entry,
    )?;

    let mut transaction = Transaction::new_with_payer(&deploy_instructions, Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &program_keypair, &upgrade_authority_keypair],
        rpc_client.get_latest_blockhash()?,
    );
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

    emit(
        &DeployedProgramView {
            program_id: program_id.to_string(),
            upgrade_authority: upgrade_authority.to_string(),
            signature: signature.to_string(),
        },
        mode,
    )
}

/// Upgrade an existing program on SphereNet
pub fn upgrade_program(
    url: &str,
    program_id_str: String,
    program_so_path: String,
    upgrade_authority: Authority,
    payer_keypair_path: String,
    spill_address_str: Option<String>,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress("🔄 Upgrading program on SphereNet...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Load keypairs and parse addresses
    let payer = read_keypair_file(&payer_keypair_path).map_err(|e| {
        eyre::eyre!(
            "Failed to read payer keypair from {}: {}",
            payer_keypair_path,
            e
        )
    })?;
    let program_id = Pubkey::from_str(&program_id_str)
        .map_err(|e| eyre::eyre!("Failed to parse program ID {}: {}", program_id_str, e))?;

    // Spill account defaults to payer if not specified
    let spill_address = if let Some(spill_str) = spill_address_str {
        Pubkey::from_str(&spill_str)
            .map_err(|e| eyre::eyre!("Failed to parse spill address {}: {}", spill_str, e))?
    } else {
        payer.pubkey()
    };

    // Get instruction authority early for display and later use
    let instruction_authority = upgrade_authority.instruction_authority_pubkey()?;

    progress(format!("Program ID: {}", program_id));
    progress(format!("Payer: {}", payer.pubkey()));
    progress(format!("Upgrade Authority: {}", instruction_authority));
    progress(format!("Spill Account: {}", spill_address));

    // Verify program exists
    match rpc_client.get_account(&program_id) {
        Ok(_) => progress("✅ Program exists"),
        Err(_) => {
            return Err(eyre::eyre!(
                "❌ Program {} does not exist! Use 'program deploy' to deploy a new program.",
                program_id
            ));
        }
    }

    // Read program .so file
    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre::eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    progress(format!("New program size: {} bytes", program_data.len()));

    // Verify program data account is large enough for the new program (fail-fast)
    progress("🔍 Checking program capacity...");
    let (programdata_address, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id());

    let programdata_account = rpc_client.get_account(&programdata_address).map_err(|e| {
        eyre::eyre!(
            "Failed to get ProgramData account {}: {}",
            programdata_address,
            e
        )
    })?;

    // Parse ProgramData account to get max_data_len
    // ProgramData layout: [account_type: 4 bytes][slot: 8 bytes][upgrade_authority: 32 bytes][actual_data...]
    // For upgradeable programs, the max_data_len is the total account size minus the metadata overhead (45 bytes)
    let programdata_metadata_len = 45; // Account type (4) + slot (8) + authority (32) + reserved (1)
    let current_max_len = programdata_account
        .data
        .len()
        .saturating_sub(programdata_metadata_len);

    progress(format!("Current max capacity: {} bytes", current_max_len));
    progress(format!(
        "Required capacity:    {} bytes",
        program_data.len()
    ));

    if program_data.len() > current_max_len {
        let additional_bytes = program_data.len() - current_max_len;

        // Build the appropriate extend command based on authority type
        let extend_cmd = match &upgrade_authority {
            crate::cli::authority::Authority::SingleSig { .. } => {
                format!(
                    "spherenet-admin program extend \\\n  \
                    --program-id {} \\\n  \
                    --bytes {} \\\n  \
                    --upgrade-authority {} \\\n  \
                    --payer {}",
                    program_id, additional_bytes, payer_keypair_path, payer_keypair_path
                )
            }
            crate::cli::authority::Authority::MultiSig { multisig, .. } => {
                format!(
                    "spherenet-admin program extend \\\n  \
                    --program-id {} \\\n  \
                    --bytes {} \\\n  \
                    --multisig {} \\\n  \
                    --multisig-authority <MEMBER_KEYPAIR> \\\n  \
                    --payer <MEMBER_KEYPAIR>",
                    program_id, additional_bytes, multisig
                )
            }
        };

        return Err(eyre::eyre!(
            "❌ Program data account is too small!\n\n\
            Current capacity: {} bytes\n\
            Required capacity: {} bytes\n\
            Need {} more bytes\n\n\
            Run this command to extend the program:\n\
            {}",
            current_max_len,
            program_data.len(),
            additional_bytes,
            extend_cmd
        ));
    }

    progress("✅ Program capacity sufficient");

    // Verify upgrade authority is whitelisted before spending lamports (fail-fast)
    let whitelist_entry = require_whitelist_entry(&rpc_client, instruction_authority)?;

    // Create and write buffer
    progress("📝 Creating buffer account...");
    let buffer_keypair = Keypair::new();
    let buffer_pubkey = buffer_keypair.pubkey();
    let buffer_size = UpgradeableLoaderState::size_of_buffer(program_data.len());
    let buffer_lamports = rpc_client.get_minimum_balance_for_rent_exemption(buffer_size)?;

    progress(format!("Buffer size: {} bytes", buffer_size));
    progress(format!("Buffer rent: {} lamports", buffer_lamports));

    // Create and initialize buffer account with PAYER as buffer authority
    // (payer can write to buffer, then upgrade authority authorizes the upgrade)
    let create_buffer_instructions = create_buffer(
        &payer.pubkey(),
        &buffer_pubkey,
        &payer.pubkey(), // Payer is buffer authority for writes
        buffer_lamports,
        program_data.len(),
    )?;

    let mut transaction =
        Transaction::new_with_payer(&create_buffer_instructions, Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &buffer_keypair],
        rpc_client.get_latest_blockhash()?,
    );
    rpc_client.send_and_confirm_transaction(&transaction)?;

    progress(format!("✅ Buffer account created: {}", buffer_pubkey));

    // Write program data to buffer in chunks (payer signs buffer writes)
    progress("📤 Writing program data to buffer...");
    write_buffer(
        &rpc_client,
        &payer,
        &payer, // Payer signs buffer writes
        &buffer_pubkey,
        &payer.pubkey(), // Buffer authority is payer
        &program_data,
    )?;

    // Transfer buffer authority to upgrade authority (required for multisig upgrades)
    progress("🔐 Transferring buffer authority...");
    let set_buffer_authority_ix = set_buffer_authority(
        &buffer_pubkey,
        &payer.pubkey(),        // Current buffer authority (payer)
        &instruction_authority, // New buffer authority (upgrade authority/vault PDA)
    );

    let mut set_authority_tx =
        Transaction::new_with_payer(&[set_buffer_authority_ix], Some(&payer.pubkey()));
    set_authority_tx.sign(&[&payer], rpc_client.get_latest_blockhash()?);
    rpc_client.send_and_confirm_transaction(&set_authority_tx)?;
    progress("✅ Buffer authority transferred to upgrade authority");

    // Upgrade program with whitelist validation
    progress("🎯 Upgrading program...");

    #[allow(deprecated)]
    let upgrade_ix = upgrade(
        &program_id,
        &buffer_pubkey,
        &instruction_authority, // Program's upgrade authority
        &spill_address,
        &whitelist_entry,
    );

    // Execute upgrade through authority (single-sig or multi-sig)
    let description = format!("Upgrade program {}", program_id);
    let result = upgrade_authority.execute_instruction(&rpc_client, upgrade_ix, &description)?;
    emit(&result, mode)
}

/// Extend a program's data account to accommodate larger programs
///
/// Uses ExtendProgramChecked instruction which is CPI-safe and works with multisig.
///
/// Note: The payer parameter is accepted but not used for the extend instruction itself.
/// The vault PDA (instruction_authority) pays for the rent increase via invoke_signed.
pub fn extend_program(
    url: &str,
    program_id_str: String,
    additional_bytes: u32,
    upgrade_authority: Authority,
    _payer_keypair_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress("🔧 Extending program data account...");

    // Initialize RPC client
    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Parse program ID
    let program_id = Pubkey::from_str(&program_id_str)
        .map_err(|e| eyre::eyre!("Failed to parse program ID {}: {}", program_id_str, e))?;

    // Get instruction authority (vault PDA for multisig, keypair pubkey for single-sig)
    let instruction_authority = upgrade_authority.instruction_authority_pubkey()?;

    progress(format!("Program ID:         {}", program_id));
    progress(format!("Additional Bytes:   {}", additional_bytes));
    progress(format!("Upgrade Authority:  {}", instruction_authority));
    progress(format!("Payer:              {}", instruction_authority));

    // Build extend_program instruction. loader-v3 7.0.0 made extend permissionless
    // (no authority account — extending only grows the data buffer; the payer
    // covers the added rent). Payer is instruction_authority (the vault PDA for
    // multisig, so the vault pays).
    let extend_ix = extend_program_ix(&program_id, Some(&instruction_authority), additional_bytes);

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Extend program {} by {} bytes",
        program_id, additional_bytes
    );
    let result = upgrade_authority.execute_instruction(&rpc_client, extend_ix, &description)?;
    emit(&result, mode)
}

/// Set (transfer) or renounce a program's upgrade authority.
///
/// The **current** authority signs (single-sig directly, or a multisig via
/// proposal). Without `make_final`, the new authority is resolved through the
/// shared multisig-safety gate: a raw pubkey is guarded against being a config
/// account / create-key, and `--new-multisig <create-key>` resolves to the
/// multisig's vault — the account that can actually sign future upgrades. With
/// `make_final = true`, the authority is set to `None`, making the program
/// permanently immutable.
///
/// Uses the unchecked `SetAuthority` (a multisig vault can't co-sign); the guard
/// provides the safety `SetAuthorityChecked` would otherwise give.
///
/// Note: after handing upgrade authority to a multisig, whitelist the vault
/// (`pw add <vault>`) so it can actually deploy/upgrade.
pub fn set_upgrade_authority(
    url: &str,
    program_id_str: String,
    current_authority: Authority,
    new_authority: Option<String>,
    new_multisig: Option<String>,
    make_final: bool,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress("🔑 Setting program upgrade authority...");

    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    let program_id = Pubkey::from_str(&program_id_str)
        .map_err(|e| eyre::eyre!("Failed to parse program ID {}: {}", program_id_str, e))?;

    let current = current_authority.instruction_authority_pubkey()?;

    // `--final` renounces upgradeability (authority → None); otherwise resolve
    // the new authority through the multisig-safety gate.
    let new = if make_final {
        None
    } else {
        Some(crate::cli::authority::resolve_target(
            &rpc_client,
            new_authority,
            new_multisig,
            "--new-authority",
            "--new-multisig",
        )?)
    };

    progress(format!("Program ID:         {}", program_id));
    progress(format!("Current Authority:  {}", current));
    match &new {
        Some(new) => progress(format!("New Authority:      {}", new)),
        None => progress("New Authority:      (none — program becomes immutable)"),
    }

    let set_ix = set_upgrade_authority_ix(&program_id, &current, new.as_ref());

    let description = match &new {
        Some(new) => format!("Set upgrade authority of {} to {}", program_id, new),
        None => format!("Make program {} immutable", program_id),
    };
    let result = current_authority.execute_instruction(&rpc_client, set_ix, &description)?;
    emit(&result, mode)
}
