use clap::{Parser, Subcommand};
use eyre::Result;

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
    },
    /// Remove a validator from the whitelist
    Remove { vote_account: String },
    /// Propose a new authority
    ProposeAuthority { new_authority: String },
    /// Accept pending authority transfer
    AcceptAuthority,
    /// Cancel pending authority transfer
    CancelAuthority,
}

#[derive(Subcommand)]
enum ProgramWhitelistAction {
    /// List all whitelisted programs
    List,
    /// Show authority account
    Auth,
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
            _ => println!("Command not yet implemented"),
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::List => pw::list()?,
            _ => println!("Command not yet implemented"),
        },
    }

    Ok(())
}
