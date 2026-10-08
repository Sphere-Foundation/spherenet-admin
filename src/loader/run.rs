//! Program deployment operations: deploy, upgrade, and extend.

use crate::authority::Authority;
use crate::cli::output::{emit, progress, subfield, OutputMode, Render};
use crate::pw::run::require_whitelist_entry;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use sha2::{Digest, Sha256};
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signature, Signer},
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

/// Result of `program write-buffer` — the staged buffer and a hash of the bytes
/// it holds, so the operator can verify the buffer against the released `.so`
/// before a feature-gated upgrade consumes it (there is no on-chain bytes pin).
#[derive(serde::Serialize)]
pub struct StagedBufferView {
    buffer: String,
    buffer_authority: String,
    byte_len: usize,
    sha256: String,
}

impl Render for StagedBufferView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Buffer staged\n");
        out.push_str(&subfield("Buffer", &self.buffer));
        out.push_str(&subfield("Buffer Authority", &self.buffer_authority));
        out.push_str(&subfield("Bytes", self.byte_len));
        // write_buffer always verifies on-chain before returning, so this hash
        // is proven against what actually landed, not just the local file.
        out.push_str(&subfield("SHA-256 (verified on-chain)", &self.sha256));
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

/// Stage program bytes into a Loader-v3 buffer and leave it under `buffer_authority`.
///
/// This is the first half of a deploy/upgrade — create the buffer, write the
/// ELF, hand the buffer to its final authority — factored out so it can also be
/// driven on its own (`program write-buffer`) to pre-stage a buffer for a later
/// feature-gated program upgrade that consumes it at runtime.
///
/// `buffer`:
/// - `None` → generate an ephemeral keypair (the `deploy`/`upgrade` case; the
///   address is used once and then drained by the deploy, so it never matters).
/// - `Some(kp)` → write to a caller-chosen address (its pubkey is baked into the
///   client as the upgrade's source-buffer `declare_id!`, so it must be known in
///   advance). Re-running against an already-created buffer resumes the writes.
///
/// The payer is the buffer's authority during the writes (it signs every chunk);
/// afterwards the buffer is transferred to `buffer_authority` (skipped when the
/// payer already is it). A Loader-v3 buffer cannot be made immutable — the loader
/// rejects a `None` buffer authority ("Buffer authority is not optional") — so
/// `buffer_authority` is where custody rests until the upgrade consumes it.
///
/// Returns the buffer's address.
fn write_buffer(
    rpc_client: &RpcClient,
    payer: &dyn Signer,
    program_data: &[u8],
    buffer: Option<Keypair>,
    buffer_authority: Pubkey,
) -> eyre::Result<Pubkey> {
    let buffer_keypair = buffer.unwrap_or_else(Keypair::new);
    let buffer_pubkey = buffer_keypair.pubkey();
    let buffer_size = UpgradeableLoaderState::size_of_buffer(program_data.len());
    let buffer_lamports = rpc_client.get_minimum_balance_for_rent_exemption(buffer_size)?;

    progress(format!("Buffer size: {} bytes", buffer_size));
    progress(format!("Buffer rent: {} lamports", buffer_lamports));

    // Reusable fixed-address buffers (Some) may already exist from a prior run:
    // - authority already == buffer_authority → a prior run finished the whole
    //   stage (transfer is the terminal step, only reached after every chunk
    //   landed), so there is nothing left to do.
    // - authority still == payer → creation landed but the stage was interrupted;
    //   skip creation and resume the (idempotent) chunk writes.
    // An ephemeral (None) buffer is a fresh random key, so this never fires.
    let existing_authority = match try_get_account(rpc_client, &buffer_pubkey) {
        Some(Some(account)) => match bincode::deserialize::<UpgradeableLoaderState>(&account.data) {
            Ok(UpgradeableLoaderState::Buffer { authority_address }) => Some(authority_address),
            _ => {
                return Err(eyre::eyre!(
                    "Account {} already exists and is not a Loader-v3 buffer",
                    buffer_pubkey
                ))
            }
        },
        _ => None,
    };

    if existing_authority == Some(Some(buffer_authority)) {
        progress(format!(
            "✅ Buffer {} already staged under {}; nothing to do",
            buffer_pubkey, buffer_authority
        ));
        verify_buffer_onchain(rpc_client, &buffer_pubkey, program_data)?;
        return Ok(buffer_pubkey);
    }

    if existing_authority.is_some() {
        progress(format!(
            "ℹ️  Buffer {} already created; resuming writes",
            buffer_pubkey
        ));
    } else {
        // Create the buffer with the PAYER as authority for the writes; the final
        // authority takes over afterwards. The on-chain deploy/upgrade only
        // requires the buffer's authority to match AT CONSUME TIME.
        progress("📝 Creating buffer account...");
        let create_buffer_instructions = create_buffer(
            &payer.pubkey(),
            &buffer_pubkey,
            &payer.pubkey(), // payer is buffer authority for writes
            buffer_lamports,
            program_data.len(),
        )?;
        // Landed-check: a just-created buffer existing at all proves our create
        // landed (fresh keypair, or the resume branch above already ruled out a
        // prior create).
        let buffer_created = || matches!(try_get_account(rpc_client, &buffer_pubkey), Some(Some(_)));
        send_with_retry(
            rpc_client,
            "Buffer creation",
            &create_buffer_instructions,
            &payer.pubkey(),
            &[payer, &buffer_keypair],
            Some(&buffer_created),
        )?;
        progress(format!("✅ Buffer account created: {}", buffer_pubkey));
    }

    // Write program data to buffer in chunks (payer signs buffer writes)
    progress("📤 Writing program data to buffer...");
    write_buffer_chunks(rpc_client, payer, &buffer_pubkey, program_data)?;

    // Read the buffer back and confirm its bytes byte-match the .so BEFORE the
    // hand-off: there is no on-chain bytes pin, and verifying while the payer is
    // still the authority keeps a bad write recoverable (re-run to overwrite,
    // or close the buffer).
    verify_buffer_onchain(rpc_client, &buffer_pubkey, program_data)?;

    // Transfer buffer authority to its final custodian. Skipped when the payer
    // IS that authority: the buffer is already theirs, and the no-op transfer
    // would cost a fee (and, for a KMS payer, a KMS round-trip).
    if payer.pubkey() == buffer_authority {
        progress("🔐 Payer is the buffer authority; already correct");
    } else {
        progress("🔐 Transferring buffer authority...");
        let set_buffer_authority_ix =
            set_buffer_authority(&buffer_pubkey, &payer.pubkey(), &buffer_authority);

        // Landed-check: once the transfer lands the payer is no longer the
        // buffer authority, so a blind re-send would fail on-chain — read the
        // buffer's recorded authority instead.
        let authority_transferred = || {
            let Some(Some(account)) = try_get_account(rpc_client, &buffer_pubkey) else {
                return false;
            };
            matches!(
                bincode::deserialize::<UpgradeableLoaderState>(&account.data),
                Ok(UpgradeableLoaderState::Buffer {
                    authority_address: Some(authority),
                }) if authority == buffer_authority
            )
        };
        send_with_retry(
            rpc_client,
            "Buffer authority transfer",
            &[set_buffer_authority_ix],
            &payer.pubkey(),
            &[payer],
            Some(&authority_transferred),
        )?;
        progress("✅ Buffer authority transferred");
    }

    Ok(buffer_pubkey)
}

/// Read a staged buffer back from chain and assert its ELF payload byte-matches
/// `program_data`. The buffer's on-chain layout is a metadata prefix followed by
/// the raw ELF (`size_of_buffer(0)` is exactly that prefix length), so the payload
/// is `data[prefix..]`. Fails closed on any mismatch — the integrity gate before a
/// feature-gated upgrade consumes the buffer (there is no on-chain bytes pin).
fn verify_buffer_onchain(
    rpc_client: &RpcClient,
    buffer: &Pubkey,
    program_data: &[u8],
) -> eyre::Result<()> {
    progress("🔎 Verifying on-chain buffer bytes...");
    let account = rpc_client
        .get_account(buffer)
        .map_err(|e| eyre::eyre!("Failed to read buffer {} for verification: {}", buffer, e))?;
    let prefix = UpgradeableLoaderState::size_of_buffer(0);
    let onchain_elf = account.data.get(prefix..).ok_or_else(|| {
        eyre::eyre!(
            "Buffer {} is too small ({} bytes) to hold a program",
            buffer,
            account.data.len()
        )
    })?;
    let expected: String = Sha256::digest(program_data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let actual: String = Sha256::digest(onchain_elf)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        return Err(eyre::eyre!(
            "❌ On-chain buffer bytes do NOT match the .so\n   buffer:   {}\n   expected: {}\n   on-chain: {}\n   The buffer is still under the payer's authority — re-run to overwrite, or close it.",
            buffer,
            expected,
            actual
        ));
    }
    progress("✅ On-chain buffer bytes match the .so");
    Ok(())
}

/// Writes program data to a buffer account in chunks.
///
/// Program data is split into MAX_WRITE_SIZE chunks (900 bytes) to avoid
/// hitting transaction size limits. The payer is the buffer's authority during
/// the writes and signs each chunk transaction alone.
///
/// Each chunk goes through [`send_with_retry`]: hundreds of sequential sends
/// should not abort on one blip (a KMS payer adds a network round-trip per
/// signature on top of the RPC ones). No landed-check is needed — re-sending
/// a chunk that actually landed is safe, `write` puts the same bytes at the
/// same offset.
fn write_buffer_chunks(
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
        progress("   Extending later is possible via `spherenet-admin program extend`.");
        progress(
            "   Consider deploying with generous --max-data-len (e.g., --max-data-len 500000)",
        );
    }

    // Verify upgrade authority is whitelisted before spending lamports (fail-fast)
    let whitelist_entry = require_whitelist_entry(&rpc_client, upgrade_authority)?;

    // Create and write the buffer, leaving it under the upgrade authority so the
    // deploy below can consume it (ephemeral buffer — drained by the deploy).
    let buffer_pubkey = write_buffer(
        &rpc_client,
        payer.as_ref(),
        &program_data,
        None,
        upgrade_authority,
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

/// Stage a program `.so` into a Loader-v3 buffer and stop (the first half of
/// `deploy`), for a later feature-gated upgrade that consumes the buffer at
/// runtime. No program is deployed and no gate is touched here.
///
/// `buffer_keypair` is an optional path to a pre-generated buffer keypair — its
/// pubkey is the buffer address, to be baked into the client as the upgrade's
/// source-buffer `declare_id!`, so it must be fixed in advance. Omit it to
/// generate an ephemeral buffer and just print the address. The buffer keypair
/// signs only the one-time `create_buffer`; it is powerless afterwards.
///
/// `buffer_authority` is where custody of the buffer rests until the upgrade
/// consumes it (a buffer cannot be made immutable — see [`write_buffer`]); use
/// the program-whitelist KMS authority. The emitted SHA-256 is the integrity
/// check to run against the released `.so` before activating the upgrade gate.
pub fn write_program_buffer(
    url: &str,
    program_so_path: String,
    buffer_keypair_path: Option<String>,
    buffer_authority_str: String,
    payer_keypair_path: String,
    mode: OutputMode,
) -> eyre::Result<()> {
    progress("📦 Staging program buffer on SphereNet...");

    let rpc_client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Payer may be a keypair file or a kms:// URI (it signs every ~900-byte
    // chunk). The buffer keypair, when provided, is a plain throwaway keypair
    // file: it signs the single create and is then powerless, so it needs no
    // KMS custody.
    let payer = crate::authority::signer::load_signer(&payer_keypair_path, "--payer")?;
    let buffer_keypair = buffer_keypair_path
        .map(|path| {
            read_keypair_file(&path)
                .map_err(|e| eyre::eyre!("Failed to read buffer keypair {}: {}", path, e))
        })
        .transpose()?;
    let buffer_authority = Pubkey::from_str(&buffer_authority_str).map_err(|e| {
        eyre::eyre!(
            "Failed to parse buffer authority {}: {}",
            buffer_authority_str,
            e
        )
    })?;

    progress(format!("Payer: {}", payer.pubkey()));
    progress(format!("Buffer Authority: {}", buffer_authority));

    let program_data = fs::read(&program_so_path)
        .map_err(|e| eyre::eyre!("Failed to read program file {}: {}", program_so_path, e))?;
    progress(format!("Program size: {} bytes", program_data.len()));

    let buffer_pubkey = write_buffer(
        &rpc_client,
        payer.as_ref(),
        &program_data,
        buffer_keypair,
        buffer_authority,
    )?;

    let sha256 = Sha256::digest(&program_data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    emit(
        &StagedBufferView {
            buffer: buffer_pubkey.to_string(),
            buffer_authority: buffer_authority.to_string(),
            byte_len: program_data.len(),
            sha256,
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

    // Create and write the buffer, leaving it under the upgrade authority so the
    // upgrade below can consume it (ephemeral buffer — drained by the upgrade).
    let buffer_pubkey = write_buffer(
        &rpc_client,
        payer.as_ref(),
        &program_data,
        None,
        instruction_authority,
    )?;

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
