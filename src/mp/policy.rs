use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use spherenet_authority::Authority;
use spherenet_monetary_policy_client::instructions::{
    UpdateBurnPercentBuilder,
    UpdateInflationRateBipsBuilder,
    UpdateLamportsPerSignatureBuilder,
    UpdateVatLamportsPerEpochBuilder,
};
use spherenet_monetary_policy_interface::{
    account_solana, program_solana,
    state::{account::MonetaryPolicyAccount, load},
};

#[derive(serde::Serialize)]
struct MonetaryPolicyView {
    program_id: String,
    account: String,
    authority: String,
    pending_authority: String,
    inflation_rate_bips: u64,
    lamports_per_signature: u64,
    burn_percent: u8,
    vat_lamports_per_epoch: u64,
}

impl Render for MonetaryPolicyView {
    fn to_text(&self) -> String {
        let sphr = |lamports: u64| lamports as f64 / LAMPORTS_PER_SOL as f64;
        let mut out = boxed_header("Monetary Policy Account");
        out.push_str(newline());
        out.push_str(&field("Program ID", &self.program_id));
        out.push_str(&field("Account Address", &self.account));
        out.push_str(newline());
        out.push_str(&field("Authority", &self.authority));
        out.push_str(&field("Pending Authority", &self.pending_authority));
        out.push_str(newline());
        out.push_str("Parameters:");
        out.push_str(newline());
        out.push_str(&subfield(
            "Inflation Rate",
            format!(
                "{} bips ({:.2}%)",
                self.inflation_rate_bips,
                self.inflation_rate_bips as f64 / 100.0
            ),
        ));
        out.push_str(&subfield(
            "Fee Per Signature",
            format!(
                "{} lamports ({:.9} SPHR)",
                self.lamports_per_signature,
                sphr(self.lamports_per_signature)
            ),
        ));
        out.push_str(&subfield("Burn Percent", format!("{}%", self.burn_percent)));
        out.push_str(&subfield(
            "VAT per Epoch",
            format!(
                "{} lamports ({:.9} SPHR)",
                self.vat_lamports_per_epoch,
                sphr(self.vat_lamports_per_epoch)
            ),
        ));
        out
    }
}

pub fn show(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&account_pubkey)?;
    let mp = load::<MonetaryPolicyAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize monetary policy account: {:?}", e))?;

    let view = MonetaryPolicyView {
        program_id: Pubkey::from(program_solana::id().to_bytes()).to_string(),
        account: account_pubkey.to_string(),
        authority: Pubkey::from(mp.authority).to_string(),
        pending_authority: Pubkey::from(mp.pending_authority).to_string(),
        inflation_rate_bips: mp.inflation_rate_bips(),
        lamports_per_signature: mp.lamports_per_signature(),
        burn_percent: mp.burn_percent(),
        vat_lamports_per_epoch: mp.vat_lamports_per_epoch(),
    };
    emit(&view, mode)
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
