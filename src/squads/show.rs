//! Multisig read commands.

use crate::squads::{self, Multisig, Permissions};
use borsh::BorshDeserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Signer};

/// Show multisig information (fetches on-chain data)
///
/// # Arguments
/// * `create_key_path` - Optional path to the create key keypair used during vault creation
/// * `multisig_str` - Optional multisig PDA address (must provide one of these)
/// * `url` - RPC URL
pub fn show_multisig(
    create_key_path: Option<String>,
    multisig_str: Option<String>,
    url: &str,
) -> eyre::Result<()> {
    let program_id = squads::types::SQUADS_PROGRAM_ID.parse::<Pubkey>()?;

    // Derive multisig PDA from either create_key or direct address
    let multisig_pda = match (create_key_path, multisig_str) {
        (Some(path), None) => {
            // Load create key and derive PDA
            let create_key = solana_sdk::signature::read_keypair_file(&path)
                .map_err(|e| eyre::eyre!("Failed to read create key '{}': {}", path, e))?;
            let (pda, _) = squads::types::get_multisig_pda(&create_key.pubkey(), &program_id);
            pda
        }
        (None, Some(address)) => {
            // Parse multisig PDA directly
            address
                .parse::<Pubkey>()
                .map_err(|e| eyre::eyre!("Invalid multisig address '{}': {}", address, e))?
        }
        _ => {
            return Err(eyre::eyre!(
                "Must provide either --create-key OR --multisig"
            ));
        }
    };

    // 3. Fetch account
    let rpc = RpcClient::new(url);
    let account = rpc.get_account(&multisig_pda).map_err(|e| {
        eyre::eyre!(
            "Failed to fetch multisig account at {}: {}",
            multisig_pda,
            e
        )
    })?;

    // 4. Deserialize account data (skip 8-byte Anchor discriminator)
    if account.data.len() < 8 {
        eyre::bail!("Invalid multisig account data (too short)");
    }
    // Use deserialize_reader to handle accounts with extra allocated space
    let mut data_slice = &account.data[8..];
    let multisig_data = Multisig::deserialize_reader(&mut data_slice)
        .map_err(|e| eyre::eyre!("Failed to deserialize multisig account: {}", e))?;

    // 5. Derive vault PDA (where funds are held)
    let (vault_pda, _vault_bump) = squads::types::get_vault_pda(&multisig_pda, 0, &program_id);
    let vault_balance_lamports = rpc.get_balance(&vault_pda).unwrap_or(0);
    let vault_balance_sol = vault_balance_lamports as f64 / 1_000_000_000.0;

    // 6. Display information
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║               Multisig Vault Information                      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("Create Key:      {}", multisig_data.create_key);
    println!("Multisig PDA:    {}", multisig_pda);
    println!();
    println!("Vault PDA:       {}", vault_pda);
    println!(
        "  Vault Balance: {:.9} SOL ({} lamports)",
        vault_balance_sol, vault_balance_lamports
    );
    println!();
    println!(
        "Threshold:         {}/{}",
        multisig_data.threshold,
        multisig_data.members.len()
    );
    println!("Next TX Index:     {}", multisig_data.transaction_index);
    println!(
        "Stale TX Index:    {}",
        multisig_data.stale_transaction_index
    );
    println!("Time Lock:         {} seconds", multisig_data.time_lock);
    println!();

    println!("Config Authority:");
    if multisig_data.config_authority == Pubkey::default() {
        println!("  Autonomous (no external authority)");
    } else {
        println!("  {}", multisig_data.config_authority);
    }
    println!();

    println!("Rent Collector:");
    match multisig_data.rent_collector {
        Some(rc) => println!("  {}", rc),
        None => println!("  Disabled"),
    }
    println!();

    println!("Members ({}):", multisig_data.members.len());
    for (i, member) in multisig_data.members.iter().enumerate() {
        let perms = format_permissions(&member.permissions);
        println!("  [{}] {} ({})", i + 1, member.key, perms);
    }
    println!();

    Ok(())
}

/// Helper to format permissions as readable string
fn format_permissions(perms: &Permissions) -> String {
    let mut parts = Vec::new();
    if perms.mask & (squads::types::Permission::Initiate as u8) != 0 {
        parts.push("Initiate");
    }
    if perms.mask & (squads::types::Permission::Vote as u8) != 0 {
        parts.push("Vote");
    }
    if perms.mask & (squads::types::Permission::Execute as u8) != 0 {
        parts.push("Execute");
    }
    if parts.is_empty() {
        "No permissions".to_string()
    } else {
        parts.join(" | ")
    }
}
