//! CLI routing and execution
//!
//! Routes parsed CLI commands to appropriate domain modules.

use crate::{cli, loader, mp, pw, utils, vw};
use clap::Parser;

use cli::commands::*;
use spherenet_authority::squads;

pub fn run() -> eyre::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ValidatorWhitelist { action } => match action {
            // Whitelist commands
            ValidatorWhitelistAction::List => vw::whitelist::list(&cli.url)?,
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
            // Authority commands
            ValidatorWhitelistAction::Auth => vw::authority::auth(&cli.url)?,
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
            MonetaryPolicyAction::Show => mp::policy::show(&cli.url)?,
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
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::List => pw::whitelist::list(&cli.url)?,
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
            ProgramWhitelistAction::Auth => pw::authority::auth(&cli.url)?,
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
            } => squads::commands::program_config_init(
                authority,
                treasury,
                creation_fee,
                initializer,
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
        Commands::Airdrop { pubkey, amount } => utils::airdrop::airdrop(&cli.url, pubkey, amount)?,
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
