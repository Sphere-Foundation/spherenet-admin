use crate::authority::Authority;
use crate::cli::output::{emit, progress, OutputMode};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_monetary_policy_client::instructions::{
    UpdateBurnPercentBuilder, UpdateInflationRateBipsBuilder, UpdateLamportsPerSignatureBuilder,
    UpdateVatLamportsPerEpochBuilder,
};
use spherenet_monetary_policy_interface::account_solana;

pub fn update_inflation_rate_bips(
    rpc_url: &str,
    new_rate_bips: u64,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Updating inflation rate:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!(
        "Authority:               {}",
        instruction_authority
    ));
    progress(format!(
        "New Rate:                {} bips ({:.2}%)",
        new_rate_bips,
        new_rate_bips as f64 / 100.0
    ));

    let instruction = UpdateInflationRateBipsBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_rate_bips(new_rate_bips)
        .instruction();

    let description = format!("Update inflation rate to {} bips", new_rate_bips);
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn update_lamports_per_signature(
    rpc_url: &str,
    new_lamports_per_signature: u64,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Updating lamports per signature:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!(
        "Authority:               {}",
        instruction_authority
    ));
    progress(format!(
        "New Fee:                 {} lamports",
        new_lamports_per_signature
    ));

    let instruction = UpdateLamportsPerSignatureBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_lamports_per_signature(new_lamports_per_signature)
        .instruction();

    let description = format!(
        "Update lamports per signature to {}",
        new_lamports_per_signature
    );
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn update_burn_percent(
    rpc_url: &str,
    new_percent: u8,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Updating burn percent:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!(
        "Authority:               {}",
        instruction_authority
    ));
    progress(format!("New Burn Percent:        {}%", new_percent));

    let instruction = UpdateBurnPercentBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_percent(new_percent)
        .instruction();

    let description = format!("Update burn percent to {}%", new_percent);
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn update_vat_lamports_per_epoch(
    rpc_url: &str,
    new_vat_lamports: u64,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Updating VAT lamports per epoch:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!(
        "Authority:               {}",
        instruction_authority
    ));
    progress(format!(
        "New VAT cost:            {} lamports",
        new_vat_lamports
    ));

    let instruction = UpdateVatLamportsPerEpochBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_vat_lamports(new_vat_lamports)
        .instruction();

    let description = format!("Update VAT lamports per epoch to {}", new_vat_lamports);
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}
