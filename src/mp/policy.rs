use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_authority::Authority;
use spherenet_monetary_policy_client::instructions::{
    UpdateBurnPercentBuilder, UpdateInflationRateBipsBuilder, UpdateLamportsPerSignatureBuilder,
};
use spherenet_monetary_policy_interface::{
    account_solana, program_solana,
    state::{account::MonetaryPolicyAccount, load},
};

pub fn show(rpc_url: &str) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&account_pubkey)?;
    let monetary_policy = load::<MonetaryPolicyAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize monetary policy account: {:?}", e))?;

    // Print monetary policy info
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║               Monetary Policy Account                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "Program ID:          {}",
        Pubkey::from(program_solana::id().to_bytes())
    );
    println!("Account Address:     {}", account_pubkey);
    println!();
    println!(
        "Authority:           {}",
        Pubkey::from(monetary_policy.authority)
    );
    println!(
        "Pending Authority:   {}",
        Pubkey::from(monetary_policy.pending_authority)
    );
    println!();
    println!("Parameters:");
    println!(
        "  Inflation Rate:    {} bips ({:.2}%)",
        monetary_policy.inflation_rate_bips(),
        monetary_policy.inflation_rate_bips() as f64 / 100.0
    );
    println!(
        "  Fee Per Signature: {} lamports ({:.6} SOL)",
        monetary_policy.lamports_per_signature(),
        monetary_policy.lamports_per_signature() as f64 / 1_000_000_000.0
    );
    println!("  Burn Percent:      {}%", monetary_policy.burn_percent());
    println!(
        "  VAT per Epoch:     {} lamports ({:.6} SOL)",
        monetary_policy.vat_lamports_per_epoch(),
        monetary_policy.vat_lamports_per_epoch() as f64 / 1_000_000_000.0
    );
    println!();

    Ok(())
}

pub fn update_inflation_rate_bips(
    rpc_url: &str,
    new_rate_bips: u64,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nUpdating inflation rate:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  Authority:               {}", instruction_authority);
    println!(
        "  New Rate:                {} bips ({:.2}%)",
        new_rate_bips,
        new_rate_bips as f64 / 100.0
    );

    // Build the instruction
    let instruction = UpdateInflationRateBipsBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_rate_bips(new_rate_bips)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Update inflation rate to {} bips", new_rate_bips);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn update_lamports_per_signature(
    rpc_url: &str,
    new_lamports_per_signature: u64,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nUpdating lamports per signature:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  Authority:               {}", instruction_authority);
    println!(
        "  New Fee:                 {} lamports",
        new_lamports_per_signature
    );

    // Build the instruction
    let instruction = UpdateLamportsPerSignatureBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_lamports_per_signature(new_lamports_per_signature)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!(
        "Update lamports per signature to {}",
        new_lamports_per_signature
    );
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}

pub fn update_burn_percent(
    rpc_url: &str,
    new_percent: u8,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nUpdating burn percent:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  Authority:               {}", instruction_authority);
    println!("  New Burn Percent:        {}%", new_percent);

    // Build the instruction
    let instruction = UpdateBurnPercentBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_percent(new_percent)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Update burn percent to {}%", new_percent);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}
