//! Utility actions: airdrop and transfer.

use crate::authority::Authority;
use crate::cli::commands::Amount;
use crate::cli::output::{emit, progress, subfield, OutputMode, Render};
use crate::squads;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    message::Message, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signer::Signer,
};
use solana_system_interface::instruction as system_instruction;
use std::str::FromStr;

#[derive(serde::Serialize)]
pub struct AirdropResult {
    pubkey: String,
    amount_sphr: f64,
    signature: String,
    new_balance_sphr: f64,
}

impl Render for AirdropResult {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ Airdrop successful!\n");
        out.push_str(&subfield("Account", &self.pubkey));
        out.push_str(&subfield("Amount", format!("{} SPHR", self.amount_sphr)));
        out.push_str(&subfield(
            "New balance",
            format!("{} SPHR", self.new_balance_sphr),
        ));
        out.push_str(&subfield("Signature", &self.signature));
        out
    }
}

/// Request an airdrop and verify it actually landed. Shared by the `airdrop`
/// command and the HTTP API — no printing (beyond progress), returns the result.
pub fn request_airdrop(
    rpc_url: &str,
    pubkey_str: String,
    amount: f64,
) -> eyre::Result<AirdropResult> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let pubkey = Pubkey::from_str(&pubkey_str)
        .map_err(|e| eyre::eyre!("Failed to parse pubkey {}: {}", pubkey_str, e))?;
    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    // Capture the balance up front so we can verify the airdrop actually landed
    // — a signature alone doesn't prove funding (the faucet may be empty, the
    // request rate-limited, or the transaction may fail on-chain).
    let before = rpc_client.get_balance(&pubkey)?;

    progress(format!(
        "Requesting airdrop of {} SPHR to {}...",
        amount, pubkey
    ));
    // Pass a FRESH blockhash. The RPC node's faucet signs the funding transfer
    // with whatever blockhash we hand it; the blockhash-less `request_airdrop`
    // sends `None`, so the faucet falls back to the bank's last confirmed
    // blockhash — often already stale by the time the transfer lands, so it
    // silently never confirms (balance unchanged). Fetching the latest blockhash
    // and requesting against it is what the client CLI does.
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let signature =
        rpc_client.request_airdrop_with_blockhash(&pubkey, lamports, &recent_blockhash)?;

    progress("Confirming airdrop...");
    // Confirm against the SAME fresh blockhash we requested with, retrying until
    // the tx confirms or that blockhash expires — this is what the client CLI
    // does. The single-shot `confirm_transaction` checks exactly once, right
    // after submit, and returns false before a faucet transfer has had time to
    // land, so it loses the race and reports a false "did not confirm".
    rpc_client
        .confirm_transaction_with_spinner(&signature, &recent_blockhash, rpc_client.commitment())
        .map_err(|e| {
            eyre::eyre!(
                "Airdrop {} did not confirm — the faucet may be empty or rate-limited: {}",
                signature,
                e
            )
        })?;

    let after = rpc_client.get_balance(&pubkey)?;
    if after <= before {
        return Err(eyre::eyre!(
            "Airdrop confirmed (signature {}) but the balance did not increase (still {} SPHR) \
             — the faucet could not fund the request.",
            signature,
            after as f64 / LAMPORTS_PER_SOL as f64
        ));
    }

    Ok(AirdropResult {
        pubkey: pubkey.to_string(),
        amount_sphr: amount,
        signature: signature.to_string(),
        new_balance_sphr: after as f64 / LAMPORTS_PER_SOL as f64,
    })
}

/// Request an airdrop for an account (testnet).
pub fn airdrop(
    rpc_url: &str,
    pubkey_str: String,
    amount: f64,
    mode: OutputMode,
) -> eyre::Result<()> {
    emit(&request_airdrop(rpc_url, pubkey_str, amount)?, mode)
}

/// Transfer SPHR to a raw address (`to`) or to a multisig's vault (`to_multisig`,
/// by create-key). Exactly one destination must be given; a multisig create-key
/// is resolved + validated so funds land in the vault, never the config account.
///
/// `--amount ALL` drains the source: single-sig only, resolved to the source
/// balance minus the actual fee of the transfer transaction (queried from the
/// RPC node, so it is correct for any signer). Rejected for multisig — the
/// amount would be baked into the proposal at creation time, but the vault
/// balance can change before the proposal executes.
pub fn transfer(
    rpc_url: &str,
    from: Authority,
    to: Option<String>,
    to_multisig: Option<String>,
    amount: Amount,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Resolve the destination (raw address or a multisig's vault) through the
    // shared multisig-safety gate.
    let destination =
        crate::authority::resolve_target(&rpc_client, to, to_multisig, "--to", "--to-multisig")?;

    // Numeric amounts were converted to lamports exactly at parse time; ALL
    // resolves here to balance − fee. The drain path also returns the
    // blockhash its fee quote was computed against; the final transaction
    // signs with it so the quote and the charge refer to the same fee state.
    let (lamports, fee_blockhash) = match (amount, &from) {
        (Amount::Lamports(lamports), _) => (lamports, None),
        (Amount::All, Authority::SingleSig { signer }) => {
            let (lamports, blockhash) = drain_lamports(&rpc_client, signer, &destination)?;
            (lamports, Some(blockhash))
        }
        (Amount::All, Authority::MultiSig { .. }) => eyre::bail!(
            "--amount ALL is not supported with --multisig: the amount would be fixed at \
             proposal creation, but the vault balance can change before the proposal \
             executes. Pass an explicit --amount instead."
        ),
    };
    let amount_sphr = lamports as f64 / LAMPORTS_PER_SOL as f64;

    progress(format!(
        "Transferring {} SPHR to {}",
        amount_sphr, destination
    ));

    // Show the source (progress → stderr).
    match &from {
        crate::authority::Authority::SingleSig { signer } => {
            progress(format!("From:     {}", signer.pubkey()));
        }
        crate::authority::Authority::MultiSig { multisig, member } => {
            progress(format!("Proposer: {}", member.pubkey()));
            progress(format!("Multisig: {}", multisig));
            // Funds come from the vault PDA, not the config account.
            let program_id =
                squads::types::SQUADS_PROGRAM_ID.parse::<solana_sdk::pubkey::Pubkey>()?;
            let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);
            let vault_balance = rpc_client.get_balance(&vault_pda).unwrap_or(0);
            progress(format!(
                "Vault:    {} ({:.9} SPHR)",
                vault_pda,
                vault_balance as f64 / LAMPORTS_PER_SOL as f64
            ));
        }
    }

    // Build system transfer instruction (from = vault PDA for multisig).
    let from_pubkey = from.instruction_authority_pubkey()?;
    let instruction = system_instruction::transfer(&from_pubkey, &destination, lamports);

    let description = format!("Transfer {} SPHR to {}", amount_sphr, destination);
    let result = from.execute_instruction_with_blockhash(
        &rpc_client,
        instruction,
        fee_blockhash,
        &description,
    )?;

    emit(&result, mode)
}

/// Resolve `--amount ALL` for a single-sig transfer: the source balance minus
/// the fee of the transfer transaction itself. The fee is queried from the RPC
/// node for the exact message that will be sent (the fee does not depend on
/// the lamport amount), so this works for any signer and signature count.
///
/// Also returns the blockhash the fee was quoted against — the caller must
/// sign the final transaction with it, so that a fee-parameter change between
/// quote and submission cannot invalidate the drained amount.
fn drain_lamports(
    rpc_client: &RpcClient,
    signer: &crate::authority::signer::AdminSigner,
    destination: &Pubkey,
) -> eyre::Result<(u64, solana_sdk::hash::Hash)> {
    let balance = rpc_client.get_balance(&signer.pubkey())?;

    // Fee-probe with a placeholder amount; only the message shape matters.
    let probe = system_instruction::transfer(&signer.pubkey(), destination, 0);
    let blockhash = rpc_client.get_latest_blockhash()?;
    let message = Message::new_with_blockhash(&[probe], Some(&signer.pubkey()), &blockhash);
    let fee = rpc_client.get_fee_for_message(&message)?;

    let lamports = balance.saturating_sub(fee);
    if lamports == 0 {
        return Err(eyre::eyre!(
            "Nothing to transfer: balance {:.9} SPHR does not exceed the transaction fee \
             of {:.9} SPHR",
            balance as f64 / LAMPORTS_PER_SOL as f64,
            fee as f64 / LAMPORTS_PER_SOL as f64
        ));
    }
    Ok((lamports, blockhash))
}
