//! CLI routing and execution
//!
//! Routes parsed CLI commands to appropriate domain modules.

use crate::cli::output::OutputMode;
use crate::{cli, loader, mp, pw, server, stake, utils, vote, vw};
use clap::Parser;

use crate::squads;
use cli::commands::*;

pub fn run() -> eyre::Result<()> {
    let cli = Cli::parse();
    let mode = cli.output;

    let result = dispatch(cli);

    // In JSON mode, surface errors as a JSON object on stderr (exit nonzero) so
    // downstream tooling never has to parse eyre's human-readable report.
    if let Err(e) = &result {
        if mode == OutputMode::Json {
            eprintln!("{}", serde_json::json!({ "error": e.to_string() }));
            std::process::exit(1);
        }
    }
    result
}

fn dispatch(cli: Cli) -> eyre::Result<()> {
    let mode = cli.output;

    match cli.command {
        Commands::ValidatorWhitelist { action } => match action {
            ValidatorWhitelistAction::Show => vw::show::show(&cli.url, mode)?,
            ValidatorWhitelistAction::Add {
                vote_account,
                start_epoch,
                end_epoch,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::run::add(&cli.url, vote_account, start_epoch, end_epoch, auth, mode)?
            }
            ValidatorWhitelistAction::Remove {
                vote_account,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::run::remove(&cli.url, vote_account, auth, mode)?
            }
            ValidatorWhitelistAction::UpdateStartEpoch {
                vote_account,
                epoch,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::run::update_start_epoch(&cli.url, vote_account, epoch, auth, mode)?
            }
            ValidatorWhitelistAction::UpdateEndEpoch {
                vote_account,
                epoch,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::run::update_end_epoch(&cli.url, vote_account, epoch, auth, mode)?
            }
            ValidatorWhitelistAction::ProposeAuthority {
                new_authority,
                new_multisig,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::auth::propose_authority(&cli.url, new_authority, new_multisig, auth, mode)?
            }
            ValidatorWhitelistAction::AcceptAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::auth::accept_authority(&cli.url, auth, mode)?
            }
            ValidatorWhitelistAction::CancelAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                vw::auth::cancel_authority(&cli.url, auth, mode)?
            }
        },
        Commands::MonetaryPolicy { action } => match action {
            MonetaryPolicyAction::Show => mp::show::show(&cli.url, mode)?,
            MonetaryPolicyAction::ProposeAuthority {
                new_authority,
                new_multisig,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::auth::propose_authority(&cli.url, new_authority, new_multisig, auth, mode)?
            }
            MonetaryPolicyAction::AcceptAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::auth::accept_authority(&cli.url, auth, mode)?
            }
            MonetaryPolicyAction::CancelAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::auth::cancel_authority(&cli.url, auth, mode)?
            }
            MonetaryPolicyAction::UpdateInflationRate {
                new_rate_bips,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::run::update_inflation_rate_bips(&cli.url, new_rate_bips, auth, mode)?
            }
            MonetaryPolicyAction::UpdateLamportsPerSignature {
                new_lamports_per_signature,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::run::update_lamports_per_signature(&cli.url, new_lamports_per_signature, auth, mode)?
            }
            MonetaryPolicyAction::UpdateBurnPercent {
                new_percent,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::run::update_burn_percent(&cli.url, new_percent, auth, mode)?
            }
            MonetaryPolicyAction::UpdateVatLamportsPerEpoch {
                new_vat_lamports,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                mp::run::update_vat_lamports_per_epoch(&cli.url, new_vat_lamports, auth, mode)?
            }
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::Show => pw::show::show(&cli.url, mode)?,
            ProgramWhitelistAction::Add {
                program_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                pw::run::add(&cli.url, program_authority, auth, mode)?
            }
            ProgramWhitelistAction::Remove {
                program_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                pw::run::remove(&cli.url, program_authority, auth, mode)?
            }
            ProgramWhitelistAction::ProposeAuthority {
                new_authority,
                new_multisig,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                pw::auth::propose_authority(&cli.url, new_authority, new_multisig, auth, mode)?
            }
            ProgramWhitelistAction::AcceptAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                pw::auth::accept_authority(&cli.url, auth, mode)?
            }
            ProgramWhitelistAction::CancelAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    authority,
                    multisig,
                    multisig_authority,
                )?;
                pw::auth::cancel_authority(&cli.url, auth, mode)?
            }
        },
        Commands::Program { action } => match action {
            ProgramAction::Deploy {
                program_so,
                program_keypair,
                upgrade_authority,
                payer,
                max_data_len,
            } => loader::run::deploy(
                &cli.url,
                program_so,
                program_keypair,
                upgrade_authority,
                payer,
                max_data_len,
            )?,
            ProgramAction::Upgrade {
                program_id,
                program_so,
                upgrade_authority,
                multisig,
                multisig_authority,
                payer,
                spill,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    upgrade_authority,
                    multisig,
                    multisig_authority,
                )?;
                loader::run::upgrade_program(&cli.url, program_id, program_so, auth, payer, spill)?
            }
            ProgramAction::Extend {
                program_id,
                bytes,
                upgrade_authority,
                multisig,
                multisig_authority,
                payer,
            } => {
                let auth = cli::authority::from_cli_args(
                    &cli.url,
                    upgrade_authority,
                    multisig,
                    multisig_authority,
                )?;
                loader::run::extend_program(&cli.url, program_id, bytes, auth, payer)?
            }
        },
        Commands::Multisig { action } => match action {
            MultisigAction::Create {
                members,
                threshold,
                create_key,
                payer,
                time_lock,
                memo,
            } => squads::run::create(
                members, threshold, create_key, payer, &cli.url, time_lock, memo, mode,
            )?,
            MultisigAction::Show { create_key } => squads::show::show(&cli.url, create_key, mode)?,
            MultisigAction::Approve {
                create_key,
                transaction_index,
                member,
            } => squads::run::approve(create_key, transaction_index, member, &cli.url, mode)?,
            MultisigAction::Execute {
                create_key,
                transaction_index,
                member,
            } => squads::run::execute(create_key, transaction_index, member, &cli.url, mode)?,
        },
        Commands::Vote { action } => match action {
            VoteAction::Show { vote_account } => vote::show::show(&cli.url, vote_account, mode)?,
            VoteAction::Create {
                vote_account,
                identity,
                authorized_voter,
                authorized_withdrawer,
                commission,
                from,
                payer,
            } => vote::run::create(
                &cli.url,
                vote_account,
                identity,
                authorized_voter,
                authorized_withdrawer,
                commission,
                from,
                payer,
            )?,
        },
        Commands::Stake { action } => match action {
            StakeAction::Show { stake_account } => {
                stake::show::show(&cli.url, stake_account, mode)?
            }
            StakeAction::Create {
                stake_account,
                amount,
                stake_authority,
                withdraw_authority,
                from,
                payer,
            } => stake::run::create(
                &cli.url,
                stake_account,
                amount,
                stake_authority,
                withdraw_authority,
                from,
                payer,
            )?,
            StakeAction::Delegate {
                stake_account,
                vote_account,
                stake_authority,
                payer,
            } => stake::run::delegate(
                &cli.url,
                stake_account,
                vote_account,
                stake_authority,
                payer,
            )?,
            StakeAction::Deactivate {
                stake_account,
                stake_authority,
                payer,
            } => stake::run::deactivate(&cli.url, stake_account, stake_authority, payer)?,
            StakeAction::Withdraw {
                stake_account,
                destination,
                amount,
                all,
                withdraw_authority,
                payer,
            } => stake::run::withdraw(
                &cli.url,
                stake_account,
                destination,
                amount,
                all,
                withdraw_authority,
                payer,
            )?,
        },
        Commands::Balance { pubkey } => utils::show::balance(&cli.url, pubkey, mode)?,
        Commands::Epoch => utils::show::epoch(&cli.url, mode)?,
        Commands::Server { port } => server::run::serve(&cli.url, port)?,
        Commands::Airdrop { pubkey, amount } => {
            utils::run::airdrop(&cli.url, pubkey, amount, mode)?
        }
        Commands::Transfer {
            to,
            to_multisig,
            amount,
            from,
            multisig,
            multisig_authority,
        } => {
            let auth = cli::authority::from_cli_args(&cli.url, from, multisig, multisig_authority)?;
            utils::run::transfer(&cli.url, auth, to, to_multisig, amount, mode)?
        }
    }

    Ok(())
}
