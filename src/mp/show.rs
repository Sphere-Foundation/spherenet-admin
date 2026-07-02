//! Show the monetary policy account: authorities and economic parameters.

use crate::cli::output::{boxed_header, emit, field, newline, subfield, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use spherenet_monetary_policy_interface::{
    account_solana, program_solana,
    state::{account::MonetaryPolicyAccount, load},
};

#[derive(serde::Serialize)]
pub struct MonetaryPolicyView {
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

/// Fetch the monetary policy account and build the view. Shared by the `show`
/// command and the HTTP API — no printing, just data.
pub fn fetch(rpc_url: &str) -> eyre::Result<MonetaryPolicyView> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&account_pubkey)?;
    let mp = load::<MonetaryPolicyAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize monetary policy account: {:?}", e))?;

    Ok(MonetaryPolicyView {
        program_id: Pubkey::from(program_solana::id().to_bytes()).to_string(),
        account: account_pubkey.to_string(),
        authority: Pubkey::from(mp.authority).to_string(),
        pending_authority: Pubkey::from(mp.pending_authority).to_string(),
        inflation_rate_bips: mp.inflation_rate_bips(),
        lamports_per_signature: mp.lamports_per_signature(),
        burn_percent: mp.burn_percent(),
        vat_lamports_per_epoch: mp.vat_lamports_per_epoch(),
    })
}

pub fn show(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    emit(&fetch(rpc_url)?, mode)
}
