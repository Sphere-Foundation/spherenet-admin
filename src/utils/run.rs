//! Utility actions: airdrop, transfer — plus the shared signer-dedup helper
//! used across command modules.

use crate::cli::output::{emit, progress, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair, signer::Signer,
};
use solana_system_interface::instruction as system_instruction;
use crate::authority::{squads, Authority, ExecutionResult};
use std::str::FromStr;

/// Deduplicate signers by pubkey, preserving order (first occurrence wins).
///
/// Several roles (funder, fee payer, authority, new account) frequently
/// collapse onto one keypair. A transaction that lists the same key twice as a
/// signer is rejected, so callers pass every role and let this drop duplicates.
pub fn dedupe_signers<'a>(signers: &[&'a Keypair]) -> Vec<&'a Keypair> {
    let mut seen: Vec<Pubkey> = Vec::with_capacity(signers.len());
    let mut out: Vec<&Keypair> = Vec::with_capacity(signers.len());
    for kp in signers {
        let pk = kp.pubkey();
        if !seen.contains(&pk) {
            seen.push(pk);
            out.push(kp);
        }
    }
    out
}

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

    progress(format!("Requesting airdrop of {} SPHR to {}...", amount, pubkey));
    let signature = rpc_client.request_airdrop(&pubkey, lamports)?;

    progress("Confirming airdrop...");
    let confirmed = rpc_client.confirm_transaction(&signature)?;
    if !confirmed {
        return Err(eyre::eyre!(
            "Airdrop {} did not confirm — the faucet may be empty or rate-limited.",
            signature
        ));
    }

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

/// Transfer SOL from one account to another
pub fn transfer(
    rpc_url: &str,
    from: Authority,
    destination_str: String,
    amount: f64,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Parse destination pubkey
    let destination = Pubkey::from_str(&destination_str).map_err(|e| {
        eyre::eyre!(
            "Failed to parse destination pubkey {}: {}",
            destination_str,
            e
        )
    })?;

    // Convert SOL to lamports
    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    println!("\nTransferring {} SOL to {}", amount, destination);

    // Display different info based on authority type
    match &from {
        crate::authority::Authority::SingleSig { keypair } => {
            println!("  From:        {}", keypair.pubkey());
        }
        crate::authority::Authority::MultiSig { multisig, member } => {
            println!("  Proposer:    {}", member.pubkey());
            println!("  Multisig:    {}", multisig);

            // Derive and show vault PDA (where funds will come from)
            let program_id =
                squads::types::SQUADS_PROGRAM_ID.parse::<solana_sdk::pubkey::Pubkey>()?;
            let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);

            // Get vault balance
            let vault_balance = rpc_client.get_balance(&vault_pda).unwrap_or(0);
            let vault_balance_sol = vault_balance as f64 / LAMPORTS_PER_SOL as f64;

            println!("  Vault PDA:   {}", vault_pda);
            println!("    Balance:   {:.9} SOL", vault_balance_sol);
        }
    }
    println!();

    // Build system transfer instruction
    // For multisig, we need the vault PDA (not the multisig PDA) as the "from" address
    let from_pubkey = from.instruction_authority_pubkey()?;
    let instruction = system_instruction::transfer(&from_pubkey, &destination, lamports);

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Transfer {} SOL to {}", amount, destination);
    let result = from.execute_instruction(&rpc_client, instruction, &description)?;

    // Only show "executed successfully" for single-sig (immediate execution)
    if matches!(result, ExecutionResult::Executed { .. }) {
        println!("\n✅ Transfer executed successfully!");
    }

    Ok(())
}
