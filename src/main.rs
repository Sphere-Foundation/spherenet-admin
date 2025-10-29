use clap::{Parser, Subcommand};
use eyre::Result;

mod airdrop;
mod consts;
mod pw;
mod vw;

#[derive(Parser)]
#[command(name = "spherenet-admin")]
#[command(about = "SphereNet administration CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validator whitelist commands
    #[command(name = "vw")]
    ValidatorWhitelist {
        #[command(subcommand)]
        action: ValidatorWhitelistAction,
    },
    /// Program whitelist commands
    #[command(name = "pw")]
    ProgramWhitelist {
        #[command(subcommand)]
        action: ProgramWhitelistAction,
    },
    /// Request an airdrop for an account
    Airdrop {
        #[arg(long)]
        keypair: String,
        #[arg(long, default_value = "1.0")]
        amount: f64,
    },
}

#[derive(Subcommand)]
enum ValidatorWhitelistAction {
    /// List all whitelisted validators
    List,
    /// Add a validator to the whitelist
    Add {
        vote_account: String,
        #[arg(long)]
        start_epoch: Option<u64>,
        #[arg(long)]
        end_epoch: Option<u64>,
        #[arg(long)]
        keypair: String,
    },
    /// Remove a validator from the whitelist
    Remove {
        vote_account: String,
        #[arg(long)]
        keypair: String,
    },
    /// Update a validator's start epoch
    UpdateStartEpoch {
        vote_account: String,
        #[arg(long)]
        epoch: u64,
        #[arg(long)]
        keypair: String,
    },
    /// Update a validator's end epoch
    UpdateEndEpoch {
        vote_account: String,
        #[arg(long)]
        epoch: u64,
        #[arg(long)]
        keypair: String,
    },
    /// Show authority account
    Auth,
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        #[arg(long)]
        keypair: String,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        #[arg(long)]
        keypair: String,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        #[arg(long)]
        keypair: String,
    },
}

#[derive(Subcommand)]
enum ProgramWhitelistAction {
    /// List all whitelisted deployer authorities
    List,
    /// Whitelist a deployer authority (who can deploy/upgrade programs)
    Add {
        program_authority: String,
        #[arg(long)]
        keypair: String,
    },
    /// Remove a deployer authority from the whitelist
    Remove {
        program_authority: String,
        #[arg(long)]
        keypair: String,
    },
    /// Show authority account
    Auth,
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        #[arg(long)]
        keypair: String,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        #[arg(long)]
        keypair: String,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        #[arg(long)]
        keypair: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ValidatorWhitelist { action } => match action {
            // Whitelist commands
            ValidatorWhitelistAction::List => vw::whitelist::list()?,
            ValidatorWhitelistAction::Add {
                vote_account,
                start_epoch,
                end_epoch,
                keypair,
            } => vw::whitelist::add(vote_account, start_epoch, end_epoch, keypair)?,
            ValidatorWhitelistAction::Remove {
                vote_account,
                keypair,
            } => vw::whitelist::remove(vote_account, keypair)?,
            ValidatorWhitelistAction::UpdateStartEpoch {
                vote_account,
                epoch,
                keypair,
            } => vw::whitelist::update_start_epoch(vote_account, epoch, keypair)?,
            ValidatorWhitelistAction::UpdateEndEpoch {
                vote_account,
                epoch,
                keypair,
            } => vw::whitelist::update_end_epoch(vote_account, epoch, keypair)?,
            // Authority commands
            ValidatorWhitelistAction::Auth => vw::authority::auth()?,
            ValidatorWhitelistAction::ProposeAuthority {
                new_authority,
                keypair,
            } => vw::authority::propose_authority(new_authority, keypair)?,
            ValidatorWhitelistAction::AcceptAuthority { keypair } => {
                vw::authority::accept_authority(keypair)?
            }
            ValidatorWhitelistAction::CancelAuthority { keypair } => {
                vw::authority::cancel_authority(keypair)?
            }
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::List => pw::whitelist::list()?,
            ProgramWhitelistAction::Add {
                program_authority,
                keypair,
            } => pw::whitelist::add(program_authority, keypair)?,
            ProgramWhitelistAction::Remove {
                program_authority,
                keypair,
            } => pw::whitelist::remove(program_authority, keypair)?,
            ProgramWhitelistAction::Auth => pw::authority::auth()?,
            ProgramWhitelistAction::ProposeAuthority {
                new_authority,
                keypair,
            } => pw::authority::propose_authority(new_authority, keypair)?,
            ProgramWhitelistAction::AcceptAuthority { keypair } => {
                pw::authority::accept_authority(keypair)?
            }
            ProgramWhitelistAction::CancelAuthority { keypair } => {
                pw::authority::cancel_authority(keypair)?
            }
        },
        Commands::Airdrop { keypair, amount } => airdrop::airdrop(keypair, amount)?,
    }

    Ok(())
}
