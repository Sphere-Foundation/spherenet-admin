//! Authority management
//!
//! Unified authority handling for single-sig and multi-sig execution, plus the
//! CLI-arg constructor ([`from_cli_args`]). Uses the top-level `squads` client
//! for the multisig proposal path.

use crate::cli::output::{progress, TxOutputView};
use crate::squads;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signer::Signer, transaction::Transaction,
};

/// Authority that can execute instructions
///
/// Signers are trait objects so each can be a local
/// [`solana_sdk::signature::Keypair`] or a [`crate::cli::kms::KmsSigner`].
pub enum Authority {
    /// Single-signature authority (direct execution)
    SingleSig { signer: Box<dyn Signer> },
    /// Multi-signature authority (creates proposals)
    MultiSig {
        multisig: Pubkey,
        member: Box<dyn Signer>,
    },
}

impl Authority {
    /// Get the pubkey to use as "from" in instructions (the actual vault that holds funds)
    ///
    /// For SingleSig: returns the signer pubkey
    /// For MultiSig: returns the vault PDA (derived from multisig with vault_index=0)
    pub fn instruction_authority_pubkey(&self) -> eyre::Result<Pubkey> {
        match self {
            Authority::SingleSig { signer } => Ok(signer.pubkey()),
            Authority::MultiSig { multisig, .. } => {
                let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;
                let (vault_pda, _) = squads::types::get_vault_pda(multisig, 0, &program_id);
                Ok(vault_pda)
            }
        }
    }

    /// Execute an instruction using this authority
    ///
    /// # Arguments
    /// * `rpc` - RPC client for blockchain interaction
    /// * `instruction` - The instruction to execute or propose
    /// * `description` - Human-readable description for logging
    ///
    /// # Returns
    /// - `TxOutputView::Executed` for single-sig (with signature)
    /// - `TxOutputView::ProposalCreated` for multi-sig (with proposal address)
    pub fn execute_instruction(
        &self,
        rpc: &RpcClient,
        instruction: Instruction,
        description: &str,
    ) -> eyre::Result<TxOutputView> {
        match self {
            Authority::SingleSig { signer } => {
                progress(format!("Executing: {}", description));

                // Build and send transaction directly. try_sign, not sign: a
                // KMS signer does a network round-trip, so failure is a normal
                // condition, not a panic.
                let recent_blockhash = rpc.get_latest_blockhash()?;
                let mut tx = Transaction::new_with_payer(&[instruction], Some(&signer.pubkey()));
                tx.try_sign(&[signer.as_ref()], recent_blockhash)
                    .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
                let signature = rpc.send_and_confirm_transaction(&tx)?;

                Ok(TxOutputView::Executed {
                    signature: signature.to_string(),
                })
            }
            // Delegate the entire proposal flow to the squads client — the
            // adapter knows nothing about Squads PDAs or wire formats.
            Authority::MultiSig { multisig, member } => {
                squads::run::propose(rpc, multisig, member.as_ref(), instruction, description)
            }
        }
    }

    /// Execute an instruction that requires **additional** signers beyond the
    /// authority — e.g. `request_whitelist_entry`, where the validator vote
    /// account must co-sign to prove control.
    ///
    /// Single-sig only: a multisig proposal cannot atomically carry an external
    /// keypair's signature, so `MultiSig` is rejected here rather than silently
    /// dropping the co-signer.
    pub fn execute_instruction_with_cosigners(
        &self,
        rpc: &RpcClient,
        instruction: Instruction,
        cosigners: &[&dyn Signer],
        description: &str,
    ) -> eyre::Result<TxOutputView> {
        match self {
            Authority::SingleSig { signer } => {
                progress(format!("Executing: {}", description));

                let recent_blockhash = rpc.get_latest_blockhash()?;
                let mut signers: Vec<&dyn Signer> = Vec::with_capacity(1 + cosigners.len());
                signers.push(signer.as_ref());
                signers.extend(cosigners.iter().copied());
                let mut tx = Transaction::new_with_payer(&[instruction], Some(&signer.pubkey()));
                tx.try_sign(&signers, recent_blockhash)
                    .map_err(|e| eyre::eyre!("Failed to sign transaction: {}", e))?;
                let signature = rpc.send_and_confirm_transaction(&tx)?;

                Ok(TxOutputView::Executed {
                    signature: signature.to_string(),
                })
            }
            Authority::MultiSig { .. } => eyre::bail!(
                "This action requires a single-sig authority: an external account (the vote \
                 account) must co-sign, which a multisig proposal cannot carry atomically. \
                 Use --authority."
            ),
        }
    }
}

/// Build an [`Authority`] from CLI arguments.
///
/// # Arguments
/// * `url` - RPC URL, used to resolve + validate a multisig create-key
/// * `authority` - Single-sig: path to authority keypair, or a `kms://` URI
/// * `multisig` - Multi-sig: the multisig's create-key (pubkey)
/// * `multisig_authority` - Multi-sig: path to member keypair, or a `kms://` URI
///
/// # Returns
/// - `Authority::SingleSig` if only `authority` is provided
/// - `Authority::MultiSig` if `multisig` and `multisig_authority` are provided.
///   The create-key is resolved and **validated** against the chain (fails if it
///   isn't a real multisig), and the resolved multisig PDA is stored.
/// - Error if neither or an invalid combination is provided
pub fn from_cli_args(
    url: &str,
    authority: Option<String>,
    multisig: Option<String>,
    multisig_authority: Option<String>,
) -> eyre::Result<Authority> {
    match (authority, multisig, multisig_authority) {
        (Some(authority_value), None, None) => {
            // Single-sig mode
            let signer = crate::cli::signer::load_signer(&authority_value, "--authority")?;
            Ok(Authority::SingleSig { signer })
        }
        (None, Some(create_key_str), Some(member_path)) => {
            // Multi-sig mode: the multisig is referenced by its create-key,
            // resolved + validated against the chain (never a raw PDA).
            let create_key = create_key_str
                .parse::<Pubkey>()
                .map_err(|e| eyre::eyre!("Invalid create-key '{}': {}", create_key_str, e))?;
            let member = crate::cli::signer::load_signer(&member_path, "--multisig-authority")?;
            let rpc = RpcClient::new(url.to_string());
            let resolved = squads::resolve(&rpc, &create_key)?;
            Ok(Authority::MultiSig {
                multisig: resolved.multisig,
                member,
            })
        }
        _ => Err(eyre::eyre!(
            "Must provide either --authority OR (--multisig + --multisig-authority)"
        )),
    }
}

/// Resolve a "target" pubkey — a transfer destination, a new authority, etc. —
/// from either a raw address or a multisig create-key. This is the shared
/// safety gate for any command that points funds or authority at an account:
///
/// - **raw address**: guarded via [`squads::classify`] — a multisig config
///   account or create-key is rejected (funds/authority would be lost or
///   unusable), directing the user to `multisig_flag`. A plain wallet or a vault
///   PDA passes through.
/// - **create-key**: resolved + validated to the multisig's vault.
///
/// `raw_flag` / `multisig_flag` name the two CLI flags, for error messages.
pub fn resolve_target(
    rpc: &RpcClient,
    raw: Option<String>,
    multisig_create_key: Option<String>,
    raw_flag: &str,
    multisig_flag: &str,
) -> eyre::Result<Pubkey> {
    match (raw, multisig_create_key) {
        (Some(addr), None) => {
            let dest = addr
                .parse::<Pubkey>()
                .map_err(|e| eyre::eyre!("Invalid pubkey '{}': {}", addr, e))?;
            match squads::classify(rpc, &dest)? {
                Some(squads::MultisigRef::ConfigAccount) => eyre::bail!(
                    "{dest} is a Squads multisig config account — use {multisig_flag} \
                     <create-key> instead (funds/authority would otherwise be lost)."
                ),
                Some(squads::MultisigRef::CreateKey) => eyre::bail!(
                    "{dest} is a multisig create-key — use {multisig_flag} {dest} to target \
                     its vault, not the create-key account."
                ),
                None => Ok(dest),
            }
        }
        (None, Some(create_key)) => {
            let create_key = create_key
                .parse::<Pubkey>()
                .map_err(|e| eyre::eyre!("Invalid create-key '{}': {}", create_key, e))?;
            Ok(squads::resolve(rpc, &create_key)?.vault)
        }
        _ => eyre::bail!("Must provide either {raw_flag} OR {multisig_flag}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::{write_keypair_file, Keypair};

    // No RPC calls are made on any path exercised here, so the URL is a
    // placeholder.
    const URL: &str = "http://localhost:1";

    // `Result::unwrap_err` needs `Authority: Debug`, which `Box<dyn Signer>`
    // can't derive — unwrap the error side manually.
    fn expect_err(result: eyre::Result<Authority>) -> eyre::Report {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(e) => e,
        }
    }

    fn temp_keypair_file(name: &str) -> (Keypair, String) {
        let keypair = Keypair::new();
        let path = std::env::temp_dir().join(format!(
            "spherenet-admin-test-{}-{}.json",
            std::process::id(),
            name
        ));
        let path = path.to_str().unwrap().to_string();
        write_keypair_file(&keypair, &path).unwrap();
        (keypair, path)
    }

    #[test]
    fn from_cli_args_rejects_no_authority() {
        let err = expect_err(from_cli_args(URL, None, None, None));
        assert!(err.to_string().contains("--authority"), "got: {err}");
    }

    #[test]
    fn from_cli_args_rejects_mixed_modes() {
        // Clap's conflicts_with normally prevents this; the constructor must
        // still refuse rather than silently pick one.
        assert!(from_cli_args(
            URL,
            Some("authority.json".into()),
            Some("11111111111111111111111111111111".into()),
            Some("member.json".into()),
        )
        .is_err());
    }

    #[test]
    fn from_cli_args_loads_single_sig_keypair() {
        let (keypair, path) = temp_keypair_file("single-sig");
        let authority = from_cli_args(URL, Some(path.clone()), None, None).unwrap();
        std::fs::remove_file(&path).unwrap();
        match authority {
            Authority::SingleSig { signer } => assert_eq!(signer.pubkey(), keypair.pubkey()),
            Authority::MultiSig { .. } => panic!("expected SingleSig"),
        }
    }

    #[test]
    fn from_cli_args_reports_missing_keypair_path() {
        let err = expect_err(from_cli_args(
            URL,
            Some("/nonexistent/authority.json".into()),
            None,
            None,
        ));
        assert!(
            err.to_string().contains("/nonexistent/authority.json"),
            "got: {err}"
        );
    }

    #[test]
    fn from_cli_args_rejects_malformed_kms_uri() {
        // Fails at URI parsing, before any KMS client or network access.
        let err = expect_err(from_cli_args(
            URL,
            Some("kms://not-a-resource-name".into()),
            None,
            None,
        ));
        assert!(err.to_string().contains("pubkey="), "got: {err}");
    }

    /// A KMS multisig member is supported; a malformed URI must still fail at
    /// parse time, before any KMS client or network access.
    #[test]
    fn from_cli_args_rejects_malformed_kms_member_uri() {
        let err = expect_err(from_cli_args(
            URL,
            None,
            Some("11111111111111111111111111111111".into()),
            Some("kms://not-a-resource-name".into()),
        ));
        assert!(err.to_string().contains("pubkey="), "got: {err}");
    }
}
