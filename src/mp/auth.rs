use crate::cli::authority::{resolve_target, Authority};
use crate::cli::output::{emit, progress, OutputMode};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_monetary_policy_client::instructions::{
    AcceptAuthorityTransferBuilder, CancelAuthorityTransferBuilder,
    InitiateAuthorityTransferBuilder,
};
use spherenet_monetary_policy_interface::account_solana;

pub fn propose_authority(
    rpc_url: &str,
    new_authority: Option<String>,
    new_multisig: Option<String>,
    authority: Authority,
    mode: OutputMode,
) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    // Resolve the new authority — a raw pubkey, or a multisig's vault (validated).
    // A multisig must be given by create-key so the authority lands on the vault
    // (the account that can actually sign), never the config account.
    let new_authority_pubkey = resolve_target(
        &rpc_client,
        new_authority,
        new_multisig,
        "--new-authority",
        "--new-multisig",
    )?;

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Proposing authority transfer:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!("Current Authority:       {}", instruction_authority));
    progress(format!("New Authority:           {}", new_authority_pubkey));

    let instruction = InitiateAuthorityTransferBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .new_authority(new_authority_pubkey)
        .instruction();

    let description = format!("Propose authority transfer to {}", new_authority_pubkey);
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn accept_authority(rpc_url: &str, authority: Authority, mode: OutputMode) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Accepting authority transfer:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!("New Authority:           {}", instruction_authority));

    let instruction = AcceptAuthorityTransferBuilder::new()
        .monetary_policy_account(account_pubkey)
        .pending_monetary_policy_authority(instruction_authority)
        .instruction();

    let description = String::from("Accept authority transfer");
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}

pub fn cancel_authority(rpc_url: &str, authority: Authority, mode: OutputMode) -> eyre::Result<()> {
    let rpc_client = RpcClient::new(rpc_url);

    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let instruction_authority = authority.instruction_authority_pubkey()?;

    progress("Cancelling authority transfer:");
    progress(format!("Monetary Policy Account: {}", account_pubkey));
    progress(format!("Authority:               {}", instruction_authority));

    let instruction = CancelAuthorityTransferBuilder::new()
        .monetary_policy_account(account_pubkey)
        .monetary_policy_authority(instruction_authority)
        .instruction();

    let description = String::from("Cancel authority transfer");
    let result = authority.execute_instruction(&rpc_client, instruction, &description)?;
    emit(&result, mode)
}
