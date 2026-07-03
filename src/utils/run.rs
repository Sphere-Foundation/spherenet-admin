//! Utility actions: airdrop, transfer — plus the shared signer-dedup helper
//! used across command modules.

use crate::cli::authority::Authority;
use crate::cli::output::{emit, progress, subfield, OutputMode, Render, TxOutputView};
use crate::squads;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair, signer::Signer,
};
use solana_system_interface::instruction as system_instruction;
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

    progress(format!(
        "Requesting airdrop of {} SPHR to {}...",
        amount, pubkey
    ));
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

/// Transfer SOL to a raw address (`to`) or to a multisig's vault (`to_multisig`,
/// by create-key). Exactly one destination must be given; a multisig create-key
/// is resolved + validated so funds land in the vault, never the config account.
pub fn transfer(
    rpc_url: &str,
    from: Authority,
    to: Option<String>,
    to_multisig: Option<String>,
    amount: f64,
) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Resolve the destination: a raw address, or a multisig's vault (validated).
    let destination = match (to, to_multisig) {
        (Some(addr), None) => {
            let dest = Pubkey::from_str(&addr)
                .map_err(|e| eyre::eyre!("Failed to parse destination pubkey {}: {}", addr, e))?;
            // A raw --to must not be a multisig config account or create-key —
            // funds would be lost / mistargeted. Vault addresses are fine.
            match squads::classify(&rpc_client, &dest)? {
                Some(squads::MultisigRef::ConfigAccount) => eyre::bail!(
                    "{dest} is a Squads program account (likely a multisig config account) — \
                     transferring here would lose the funds. Use --to-multisig <create-key> \
                     to fund the vault."
                ),
                Some(squads::MultisigRef::CreateKey) => eyre::bail!(
                    "{dest} is the create-key of an existing multisig — funds would land on the \
                     create-key account, not the vault. Use --to-multisig {dest} instead."
                ),
                None => {}
            }
            dest
        }
        (None, Some(create_key)) => {
            let create_key = Pubkey::from_str(&create_key)
                .map_err(|e| eyre::eyre!("Invalid create-key '{}': {}", create_key, e))?;
            squads::resolve(&rpc_client, &create_key)?.vault
        }
        _ => return Err(eyre::eyre!("Must provide either --to OR --to-multisig")),
    };

    // Convert SOL to lamports
    let lamports = (amount * LAMPORTS_PER_SOL as f64) as u64;

    println!("\nTransferring {} SOL to {}", amount, destination);

    // Display different info based on authority type
    match &from {
        crate::cli::authority::Authority::SingleSig { keypair } => {
            println!("  From:        {}", keypair.pubkey());
        }
        crate::cli::authority::Authority::MultiSig { multisig, member } => {
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
    if matches!(result, TxOutputView::Executed { .. }) {
        println!("\n✅ Transfer executed successfully!");
    }

    Ok(())
}

