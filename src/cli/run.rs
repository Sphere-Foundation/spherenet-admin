//! CLI routing and execution
//!
//! Routes parsed CLI commands to appropriate domain modules.

use crate::cli::output::OutputMode;
use crate::{cli, loader, mp, pw, stake, utils, vote, vw};
use clap::Parser;

use cli::commands::*;
use spherenet_authority::squads;

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
            ValidatorWhitelistAction::Show => vw::whitelist::show(&cli.url, mode)?,
            ValidatorWhitelistAction::Add {
                vote_account,
                start_epoch,
                end_epoch,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::whitelist::add(&cli.url, vote_account, start_epoch, end_epoch, auth)?
            }
            ValidatorWhitelistAction::Remove {
                vote_account,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::whitelist::remove(&cli.url, vote_account, auth)?
            }
            ValidatorWhitelistAction::UpdateStartEpoch {
                vote_account,
                epoch,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::whitelist::update_start_epoch(&cli.url, vote_account, epoch, auth)?
            }
            ValidatorWhitelistAction::UpdateEndEpoch {
                vote_account,
                epoch,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::whitelist::update_end_epoch(&cli.url, vote_account, epoch, auth)?
            }
            ValidatorWhitelistAction::ProposeAuthority {
                new_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::authority::propose_authority(&cli.url, new_authority, auth)?
            }
            ValidatorWhitelistAction::AcceptAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::authority::accept_authority(&cli.url, auth)?
            }
            ValidatorWhitelistAction::CancelAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                vw::authority::cancel_authority(&cli.url, auth)?
            }
        },
        Commands::MonetaryPolicy { action } => match action {
            MonetaryPolicyAction::Show => mp::policy::show(&cli.url, mode)?,
            MonetaryPolicyAction::Create { authority, payer } => {
                mp::create::create(&cli.url, authority, payer)?
            }
            MonetaryPolicyAction::Auth => mp::authority::auth(&cli.url)?,
            MonetaryPolicyAction::ProposeAuthority {
                new_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::authority::propose_authority(&cli.url, new_authority, auth)?
            }
            MonetaryPolicyAction::AcceptAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::authority::accept_authority(&cli.url, auth)?
            }
            MonetaryPolicyAction::CancelAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::authority::cancel_authority(&cli.url, auth)?
            }
            MonetaryPolicyAction::UpdateInflationRate {
                new_rate_bips,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::policy::update_inflation_rate_bips(&cli.url, new_rate_bips, auth)?
            }
            MonetaryPolicyAction::UpdateLamportsPerSignature {
                new_lamports_per_signature,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::policy::update_lamports_per_signature(
                    &cli.url,
                    new_lamports_per_signature,
                    auth,
                )?
            }
            MonetaryPolicyAction::UpdateBurnPercent {
                new_percent,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::policy::update_burn_percent(&cli.url, new_percent, auth)?
            }
            MonetaryPolicyAction::UpdateVatLamportsPerEpoch {
                new_vat_lamports,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                mp::policy::update_vat_lamports_per_epoch(&cli.url, new_vat_lamports, auth)?
            }
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::Show => pw::whitelist::show(&cli.url, mode)?,
            ProgramWhitelistAction::Add {
                program_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                pw::whitelist::add(&cli.url, program_authority, auth)?
            }
            ProgramWhitelistAction::Remove {
                program_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                pw::whitelist::remove(&cli.url, program_authority, auth)?
            }
            ProgramWhitelistAction::ProposeAuthority {
                new_authority,
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                pw::authority::propose_authority(&cli.url, new_authority, auth)?
            }
            ProgramWhitelistAction::AcceptAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                pw::authority::accept_authority(&cli.url, auth)?
            }
            ProgramWhitelistAction::CancelAuthority {
                authority,
                multisig,
                multisig_authority,
            } => {
                let auth =
                    cli::authority_builder::from_cli_args(authority, multisig, multisig_authority)?;
                pw::authority::cancel_authority(&cli.url, auth)?
            }
        },
        Commands::Program { action } => match action {
            ProgramAction::Deploy {
                program_so,
                program_keypair,
                upgrade_authority,
                payer,
                max_data_len,
            } => loader::deploy::deploy(
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
                let auth = cli::authority_builder::from_cli_args(
                    upgrade_authority,
                    multisig,
                    multisig_authority,
                )?;
                loader::upgrade::upgrade_program(
                    &cli.url, program_id, program_so, auth, payer, spill,
                )?
            }
            ProgramAction::Extend {
                program_id,
                bytes,
                upgrade_authority,
                multisig,
                multisig_authority,
                payer,
            } => {
                let auth = cli::authority_builder::from_cli_args(
                    upgrade_authority,
                    multisig,
                    multisig_authority,
                )?;
                loader::extend::extend_program(&cli.url, program_id, bytes, auth, payer)?
            }
        },
        Commands::Multisig { action } => match action {
            MultisigAction::ProgramConfigInit {
                authority,
                treasury,
                creation_fee,
                initializer,
                payer,
            } => squads::commands::program_config_init(
                authority,
                treasury,
                creation_fee,
                initializer,
                payer,
                &cli.url,
            )?,
            MultisigAction::Create {
                members,
                threshold,
                create_key,
                payer,
                time_lock,
                memo,
            } => squads::commands::create(
                members, threshold, create_key, payer, &cli.url, time_lock, memo,
            )?,
            MultisigAction::Show {
                create_key,
                multisig,
            } => squads::commands::show_multisig(create_key, multisig, &cli.url)?,
            MultisigAction::Approve {
                multisig,
                transaction_index,
                member,
            } => squads::commands::approve_proposal(multisig, transaction_index, member, &cli.url)?,
            MultisigAction::Execute {
                multisig,
                transaction_index,
                member,
            } => squads::commands::execute_proposal(multisig, transaction_index, member, &cli.url)?,
        },
        Commands::Vote { action } => match action {
            VoteAction::Show { vote_account } => vote::show::show(&cli.url, vote_account)?,
            VoteAction::Create {
                vote_account,
                identity,
                authorized_voter,
                authorized_withdrawer,
                commission,
                from,
                payer,
            } => vote::create::create(
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
            StakeAction::Show { stake_account } => stake::show::show(&cli.url, stake_account)?,
            StakeAction::Create {
                stake_account,
                amount,
                stake_authority,
                withdraw_authority,
                from,
                payer,
            } => stake::create::create(
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
            } => stake::delegate::delegate(
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
            } => stake::deactivate::deactivate(&cli.url, stake_account, stake_authority, payer)?,
            StakeAction::Withdraw {
                stake_account,
                destination,
                amount,
                all,
                withdraw_authority,
                payer,
            } => stake::withdraw::withdraw(
                &cli.url,
                stake_account,
                destination,
                amount,
                all,
                withdraw_authority,
                payer,
            )?,
        },
        Commands::Balance { pubkey } => utils::balance::balance(&cli.url, pubkey, mode)?,
        Commands::Epoch => utils::epoch::epoch(&cli.url, mode)?,
        Commands::Airdrop { pubkey, amount } => {
            utils::airdrop::airdrop(&cli.url, pubkey, amount, mode)?
        }
        Commands::Transfer {
            destination,
            amount,
            from,
            multisig,
            multisig_authority,
        } => {
            let auth = cli::authority_builder::from_cli_args(from, multisig, multisig_authority)?;
            utils::transfer::transfer(&cli.url, auth, destination, amount)?
        }
    }

    Ok(())
}
