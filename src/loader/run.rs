//! Program deployment operations: deploy, upgrade, and extend.

use crate::authority::Authority;
use crate::cli::output::{emit, progress, subfield, OutputMode, Render};
use crate::pw::run::require_whitelist_entry;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
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

/// How many times to attempt each transaction send before giving up.
const SEND_ATTEMPTS: u32 = 3;

/// Builds, signs, and sends a transaction, retrying failures with a short
/// growing backoff and a fresh blockhash per attempt. `label` names the send
/// in progress and error messages.
///
/// A send that reports failure may still have landed (included on chain, but
/// the confirmation was lost). The idempotent buffer chunk writes can simply
/// re-send, but retrying a one-off send (create buffer, transfer buffer
/// authority, deploy) would then fail on-chain even though the work is done —
/// aborting a deploy whose buffer took hundreds of signatures to write.
/// `landed` closes that gap: it is consulted after each failed attempt, and
/// when it confirms the intended state is already on chain the send counts as
/// a success. It must answer definitively — an RPC error while checking means
/// "did not land", never "maybe". The signature returned on that path is the
/// most recent attempt's, which in a lost-confirmation race may not be the
/// attempt that actually landed (best-effort, for display only).
fn send_with_retry(
    rpc_client: &RpcClient,
    label: &str,
    instructions: &[Instruction],
    fee_payer: &Pubkey,
    signers: &[&dyn Signer],
    landed: Option<&dyn Fn() -> bool>,
) -> eyre::Result<Signature> {
    let mut last_signature: Option<Signature> = None;
    let mut attempt: u32 = 1;
    loop {
        let mut send_once = || -> eyre::Result<Signature> {
            let mut transaction = Transaction::new_with_payer(instructions, Some(fee_payer));
            transaction
                .try_sign(&signers.to_vec(), rpc_client.get_latest_blockhash()?)
                .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
            last_signature = Some(transaction.signatures[0]);
            Ok(rpc_client.send_and_confirm_transaction(&transaction)?)
        };
        let result = send_once();
        match result {
            Ok(signature) => return Ok(signature),
            Err(e) => {
                if let (Some(landed), Some(signature)) = (landed, last_signature) {
                    if landed() {
                        progress(format!(
                            "⚠️  {label} reported an error ({e}) but the change is on chain; continuing"
                        ));
                        return Ok(signature);
                    }
                }
                if attempt < SEND_ATTEMPTS {
                    progress(format!(
                        "⚠️  {label} attempt {attempt} failed ({e}); retrying..."
                    ));
                    std::thread::sleep(std::time::Duration::from_millis(500 * u64::from(attempt)));
                    attempt += 1;
                } else {
                    return Err(
                        e.wrap_err(format!("{label} failed after {SEND_ATTEMPTS} attempts"))
                    );
                }
            }
        }
    }
}

/// Definitive account read for [`send_with_retry`] landed-checks: `Some(_)`
/// carries the RPC's actual answer (account present or definitively absent),
/// `None` means the read itself failed and nothing can be concluded.
fn try_get_account(rpc_client: &RpcClient, pubkey: &Pubkey) -> Option<Option<Account>> {
    rpc_client
        .get_account_with_commitment(pubkey, rpc_client.commitment())
        .ok()
        .map(|response| response.value)
}

/// Writes program data to a buffer account in chunks.
///
/// Program data is split into MAX_WRITE_SIZE chunks (900 bytes) to avoid
/// hitting transaction size limits. The payer is the buffer's authority during
/// the writes and signs each chunk transaction alone; the real upgrade
/// authority takes over the buffer afterwards via `set_buffer_authority`
/// (unless the payer already is that authority).
///
/// Each chunk goes through [`send_with_retry`]: hundreds of sequential sends
/// should not abort on one blip (a KMS payer adds a network round-trip per
/// signature on top of the RPC ones). No landed-check is needed — re-sending
/// a chunk that actually landed is safe, `write` puts the same bytes at the
/// same offset.
fn write_buffer(
    rpc_client: &RpcClient,
    payer: &dyn Signer,
    buffer: &Pubkey,
    program_data: &[u8],
) -> eyre::Result<()> {
    let chunks: Vec<_> = program_data.chunks(MAX_WRITE_SIZE).collect();
    let total_chunks = chunks.len();

    for (i, chunk) in chunks.into_iter().enumerate() {
        let offset = i * MAX_WRITE_SIZE;
        let write_ix = write(buffer, &payer.pubkey(), offset as u32, chunk.to_vec());
        send_with_retry(
            rpc_client,
            &format!("Chunk {}/{}", i + 1, total_chunks),
            &[write_ix],
            &payer.pubkey(),
            &[payer],
            None,
        )?;

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
///
/// All signer arguments accept a keypair path or a `kms://` URI. The payer
/// writes the buffer chunks (one signature per ~900-byte chunk — a network
/// round-trip each for a KMS payer); the program keypair and the upgrade
/// authority each sign exactly once, on the final deploy transaction.
///
/// Deploy is single-sig only: the deploy transaction needs the program
/// keypair's co-signature, which a multisig proposal cannot carry atomically
/// (same constraint as `vw request`).
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

    // Load signers — each may be a keypair file or a kms:// URI
    let payer = crate::authority::signer::load_signer(&payer_keypair_path, "--payer")?;
    let program_keypair =
        crate::authority::signer::load_signer(&program_keypair_path, "--program-keypair")?;
    let program_id = program_keypair.pubkey();
    let upgrade_authority_signer =
        crate::authority::signer::load_signer(&upgrade_authority_path, "--upgrade-authority")?;
    let upgrade_authority = upgrade_authority_signer.pubkey();

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

    // Create and initialize buffer account with PAYER as buffer authority —
    // the same shape as `upgrade_program`: the payer signs the chunk writes,
    // then hands the buffer to the upgrade authority, which signs exactly
    // once regardless of program size. The on-chain deploy only requires the
    // buffer's authority to match the upgrade authority AT DEPLOY TIME.
    let create_buffer_instructions = create_buffer(
        &payer.pubkey(),
        &buffer_pubkey,
        &payer.pubkey(), // Payer is buffer authority for writes
        buffer_lamports,
        program_data.len(),
    )?;

    // Landed-check: the buffer keypair is freshly generated, so the account
    // existing at all proves our create landed.
    let buffer_created = || matches!(try_get_account(&rpc_client, &buffer_pubkey), Some(Some(_)));
    send_with_retry(
        &rpc_client,
        "Buffer creation",
        &create_buffer_instructions,
        &payer.pubkey(),
        &[payer.as_ref(), &buffer_keypair],
        Some(&buffer_created),
    )?;

    progress(format!("✅ Buffer account created: {}", buffer_pubkey));

    // Write program data to buffer in chunks (payer signs buffer writes)
    progress("📤 Writing program data to buffer...");
    write_buffer(&rpc_client, payer.as_ref(), &buffer_pubkey, &program_data)?;

    // Transfer buffer authority to the upgrade authority (unchecked variant —
    // the new authority does not sign here; it signs the deploy itself).
    // Skipped when the payer IS the upgrade authority: the buffer is already
    // theirs, and the no-op transfer would cost a fee (and, for a KMS payer,
    // a KMS round-trip).
    if payer.pubkey() == upgrade_authority {
        progress("🔐 Payer is the upgrade authority; buffer authority already correct");
    } else {
        progress("🔐 Transferring buffer authority...");
        let set_buffer_authority_ix =
            set_buffer_authority(&buffer_pubkey, &payer.pubkey(), &upgrade_authority);

        // Landed-check: once the transfer lands the payer is no longer the
        // buffer authority, so a blind re-send would fail on-chain — read the
        // buffer's recorded authority instead.
        let authority_transferred = || {
            let Some(Some(account)) = try_get_account(&rpc_client, &buffer_pubkey) else {
                return false;
            };
            matches!(
                bincode::deserialize::<UpgradeableLoaderState>(&account.data),
                Ok(UpgradeableLoaderState::Buffer {
                    authority_address: Some(authority),
                }) if authority == upgrade_authority
            )
        };
        send_with_retry(
            &rpc_client,
            "Buffer authority transfer",
            &[set_buffer_authority_ix],
            &payer.pubkey(),
            &[payer.as_ref()],
            Some(&authority_transferred),
        )?;
        progress("✅ Buffer authority transferred to upgrade authority");
    }

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

    // Payer, program keypair, and upgrade authority each sign once (deduped —
    // several of these roles frequently collapse onto one key).
    let signers = crate::authority::signer::dedupe_signers(&[
        payer.as_ref(),
        program_keypair.as_ref(),
        upgrade_authority_signer.as_ref(),
    ]);

    // Landed-check: deploy drains this run's freshly created buffer to zero
    // lamports, so the buffer definitively disappearing proves the deploy
    // landed. ("Program account exists" would false-positive on a program
    // that was already deployed before this run.)
    let deploy_landed = || matches!(try_get_account(&rpc_client, &buffer_pubkey), Some(None));
    let signature = send_with_retry(
        &rpc_client,
        "Deploy",
        &deploy_instructions,
        &payer.pubkey(),
        &signers,
        Some(&deploy_landed),
    )?;

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

    // Load signers and parse addresses. The payer may be a keypair file or a
    // kms:// URI — note it signs every ~900-byte buffer-write chunk (a network
    // round-trip per chunk for a KMS payer); the upgrade AUTHORITY signs once.
    let payer = crate::authority::signer::load_signer(&payer_keypair_path, "--payer")?;
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
            crate::authority::Authority::SingleSig { .. } => {
                format!(
                    "spherenet-admin program extend \\\n  \
                    --program-id {} \\\n  \
                    --bytes {} \\\n  \
                    --upgrade-authority {} \\\n  \
                    --payer {}",
                    program_id, additional_bytes, payer_keypair_path, payer_keypair_path
                )
            }
            crate::authority::Authority::MultiSig { multisig, .. } => {
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

    // Landed-check: the buffer keypair is freshly generated, so the account
    // existing at all proves our create landed.
    let buffer_created = || matches!(try_get_account(&rpc_client, &buffer_pubkey), Some(Some(_)));
    send_with_retry(
        &rpc_client,
        "Buffer creation",
        &create_buffer_instructions,
        &payer.pubkey(),
        &[payer.as_ref(), &buffer_keypair],
        Some(&buffer_created),
    )?;

    progress(format!("✅ Buffer account created: {}", buffer_pubkey));

    // Write program data to buffer in chunks (payer signs buffer writes)
    progress("📤 Writing program data to buffer...");
    write_buffer(&rpc_client, payer.as_ref(), &buffer_pubkey, &program_data)?;

    // Transfer buffer authority to upgrade authority (required for multisig
    // upgrades). Skipped when the payer IS the upgrade authority: the buffer
    // is already theirs, and the no-op transfer would cost a fee (and, for a
    // KMS payer, a KMS round-trip).
    if payer.pubkey() == instruction_authority {
        progress("🔐 Payer is the upgrade authority; buffer authority already correct");
    } else {
        progress("🔐 Transferring buffer authority...");
        let set_buffer_authority_ix = set_buffer_authority(
            &buffer_pubkey,
            &payer.pubkey(),        // Current buffer authority (payer)
            &instruction_authority, // New buffer authority (upgrade authority/vault PDA)
        );

        // Landed-check: once the transfer lands the payer is no longer the
        // buffer authority, so a blind re-send would fail on-chain — read the
        // buffer's recorded authority instead.
        let authority_transferred = || {
            let Some(Some(account)) = try_get_account(&rpc_client, &buffer_pubkey) else {
                return false;
            };
            matches!(
                bincode::deserialize::<UpgradeableLoaderState>(&account.data),
                Ok(UpgradeableLoaderState::Buffer {
                    authority_address: Some(authority),
                }) if authority == instruction_authority
            )
        };
        send_with_retry(
            &rpc_client,
            "Buffer authority transfer",
            &[set_buffer_authority_ix],
            &payer.pubkey(),
            &[payer.as_ref()],
            Some(&authority_transferred),
        )?;
        progress("✅ Buffer authority transferred to upgrade authority");
    }

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
        Some(crate::authority::resolve_target(
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
