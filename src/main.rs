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
    /// Show authority account
    Auth,
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
    /// List all whitelisted programs
    List,
    /// Add a program to the whitelist
    Add { program_id: String },
    /// Remove a program from the whitelist
    Remove { program_id: String },
    /// Propose a new authority
    ProposeAuthority { new_authority: String },
    /// Accept pending authority transfer
    AcceptAuthority,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ValidatorWhitelist { action } => match action {
            ValidatorWhitelistAction::List => vw::list()?,
            ValidatorWhitelistAction::Auth => vw::auth()?,
            ValidatorWhitelistAction::Add {
                vote_account,
                start_epoch,
                end_epoch,
                keypair,
            } => vw::add(vote_account, start_epoch, end_epoch, keypair)?,
            ValidatorWhitelistAction::Remove {
                vote_account,
                keypair,
            } => vw::remove(vote_account, keypair)?,
            ValidatorWhitelistAction::UpdateStartEpoch {
                vote_account,
                epoch,
                keypair,
            } => vw::update_start_epoch(vote_account, epoch, keypair)?,
            ValidatorWhitelistAction::UpdateEndEpoch {
                vote_account,
                epoch,
                keypair,
            } => vw::update_end_epoch(vote_account, epoch, keypair)?,
            ValidatorWhitelistAction::ProposeAuthority {
                new_authority,
                keypair,
            } => vw::propose_authority(new_authority, keypair)?,
            ValidatorWhitelistAction::AcceptAuthority { keypair } => {
                vw::accept_authority(keypair)?
            }
            ValidatorWhitelistAction::CancelAuthority { keypair } => {
                vw::cancel_authority(keypair)?
            }
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::List => pw::list()?,
            _ => println!("Command not yet implemented"),
        },
        Commands::Airdrop { keypair, amount } => airdrop::airdrop(keypair, amount)?,
    }

    Ok(())
}
