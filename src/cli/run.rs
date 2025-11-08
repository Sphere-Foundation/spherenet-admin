//! CLI routing and execution
//!
//! Routes parsed CLI commands to appropriate domain modules.

use crate::{cli, loader, pw, squads, utils, vw};
use clap::Parser;

use cli::commands::*;
use cli::Authority;

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
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::whitelist::add(&cli.url, vote_account, start_epoch, end_epoch, auth)?
            }
            ValidatorWhitelistAction::Remove {
                vote_account,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::whitelist::remove(&cli.url, vote_account, auth)?
            }
            ValidatorWhitelistAction::UpdateStartEpoch {
                vote_account,
                epoch,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::whitelist::update_start_epoch(&cli.url, vote_account, epoch, auth)?
            }
            ValidatorWhitelistAction::UpdateEndEpoch {
                vote_account,
                epoch,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::whitelist::update_end_epoch(&cli.url, vote_account, epoch, auth)?
            }
            // Authority commands
            ValidatorWhitelistAction::Auth => vw::authority::auth(&cli.url)?,
            ValidatorWhitelistAction::ProposeAuthority {
                new_authority,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::authority::propose_authority(&cli.url, new_authority, auth)?
            }
            ValidatorWhitelistAction::AcceptAuthority {
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::authority::accept_authority(&cli.url, auth)?
            }
            ValidatorWhitelistAction::CancelAuthority {
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                vw::authority::cancel_authority(&cli.url, auth)?
            }
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::List => pw::whitelist::list(&cli.url)?,
            ProgramWhitelistAction::Add {
                program_authority,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                pw::whitelist::add(&cli.url, program_authority, auth)?
            }
            ProgramWhitelistAction::Remove {
                program_authority,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                pw::whitelist::remove(&cli.url, program_authority, auth)?
            }
            ProgramWhitelistAction::Auth => pw::authority::auth(&cli.url)?,
            ProgramWhitelistAction::ProposeAuthority {
                new_authority,
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                pw::authority::propose_authority(&cli.url, new_authority, auth)?
            }
            ProgramWhitelistAction::AcceptAuthority {
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
                pw::authority::accept_authority(&cli.url, auth)?
            }
            ProgramWhitelistAction::CancelAuthority {
                authority,
                vault,
                multisig_authority,
            } => {
                let auth = Authority::from_cli_args(authority, vault, multisig_authority)?;
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
                vault,
                multisig_authority,
                payer,
                spill,
            } => {
                let auth = Authority::from_cli_args(upgrade_authority, vault, multisig_authority)?;
                loader::upgrade::upgrade_program(
                    &cli.url, program_id, program_so, auth, payer, spill,
                )?
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
        },
        Commands::Airdrop { keypair, amount } => {
            utils::airdrop::airdrop(&cli.url, keypair, amount)?
        }
    }

    Ok(())
}
