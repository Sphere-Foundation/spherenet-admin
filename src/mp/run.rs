use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use crate::cli::authority::Authority;
use spherenet_monetary_policy_client::instructions::{
    UpdateBurnPercentBuilder, UpdateInflationRateBipsBuilder, UpdateLamportsPerSignatureBuilder,
    UpdateVatLamportsPerEpochBuilder,
};
use spherenet_monetary_policy_interface::account_solana;

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

pub fn update_vat_lamports_per_epoch(
    rpc_url: &str,
    new_vat_lamports: u64,
    authority: Authority,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Get the monetary policy account
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    let instruction_authority = authority.instruction_authority_pubkey()?;

    println!("\nUpdating VAT lamports per epoch:");
    println!("  Monetary Policy Account: {}", account_pubkey);
    println!("  Authority:               {}", instruction_authority);
    println!("  New VAT cost:            {} lamports", new_vat_lamports);

    // Build the instruction
    let instruction = UpdateVatLamportsPerEpochBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_vat_lamports(new_vat_lamports)
        .instruction();

    // Execute instruction through authority (single-sig or multi-sig)
    let description = format!("Update VAT lamports per epoch to {}", new_vat_lamports);
    authority.execute_instruction(&rpc_client, instruction, &description)?;

    Ok(())
}
