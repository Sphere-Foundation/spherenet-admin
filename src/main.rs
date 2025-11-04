use clap::{Parser, Subcommand};

mod airdrop;
mod authority;
mod loader;
mod pw;
mod squads;
mod vw;

/// Default RPC_URL
pub const RPC_URL: &str = "https://api.testnet.sphere.net";

#[derive(Parser)]
#[command(name = "spherenet-admin")]
#[command(about = "SphereNet administration CLI", long_about = None)]
struct Cli {
    /// RPC URL to connect to
    #[arg(long, global = true, default_value = RPC_URL)]
    url: String,

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
    /// Program deployment commands
    Program {
        #[command(subcommand)]
        action: ProgramAction,
    },
    /// Multisig vault management commands
    Multisig {
        #[command(subcommand)]
        action: MultisigAction,
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
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Remove a validator from the whitelist
    Remove {
        vote_account: String,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Update a validator's start epoch
    UpdateStartEpoch {
        vote_account: String,
        #[arg(long)]
        epoch: u64,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Update a validator's end epoch
    UpdateEndEpoch {
        vote_account: String,
        #[arg(long)]
        epoch: u64,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Show authority account
    Auth,
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
}

#[derive(Subcommand)]
enum ProgramWhitelistAction {
    /// List all whitelisted deployer authorities
    List,
    /// Whitelist a deployer authority (who can deploy/upgrade programs)
    Add {
        program_authority: String,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Remove a deployer authority from the whitelist
    Remove {
        program_authority: String,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Show authority account
    Auth,
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        #[arg(long = "authority", alias = "auth")]
        authority: String,
    },
}

#[derive(Subcommand)]
enum ProgramAction {
    /// Deploy a program to SphereNet
    Deploy {
        /// Path to the program .so file
        #[arg(long)]
        program_so: String,

        /// Path to the program keypair (the program ID will be derived from this)
        #[arg(long)]
        program_keypair: String,

        /// Path to upgrade authority keypair (must sign deployment)
        #[arg(long)]
        upgrade_authority: String,

        /// Payer keypair path (defaults to solana config default keypair)
        #[arg(long)]
        payer: String,

        /// Maximum program data length (optional, defaults to program size)
        #[arg(long)]
        max_data_len: Option<usize>,
    },
    /// Upgrade an existing program on SphereNet
    Upgrade {
        /// Program ID of the existing program to upgrade
        #[arg(long)]
        program_id: String,

        /// Path to the new program .so file
        #[arg(long)]
        program_so: String,

        /// Path to upgrade authority keypair (must match program's current authority)
        #[arg(long)]
        upgrade_authority: String,

        /// Payer keypair path
        #[arg(long)]
        payer: String,

        /// Spill account (where excess buffer rent is returned, defaults to payer)
        #[arg(long)]
        spill: Option<String>,
    },
}

#[derive(Subcommand)]
enum MultisigAction {
    /// Initialize Squads program config (one-time setup)
    ProgramConfigInit {
        /// Pubkey that will control the program config
        #[arg(long)]
        authority: String,

        /// Pubkey where multisig creation fees are sent
        #[arg(long)]
        treasury: String,

        /// Fee in lamports charged for creating a multisig
        #[arg(long, default_value = "0")]
        creation_fee: u64,

        /// Path to INITIALIZER keypair (hardcoded in program)
        #[arg(long)]
        initializer: String,
    },
    /// Create a new multisig vault
    Create {
        /// Comma-separated list of member pubkeys
        #[arg(long)]
        members: String,

        /// Number of approvals required (e.g., 2 for 2-of-3)
        #[arg(long)]
        threshold: u16,

        /// Path to create key keypair (unique ID for this multisig)
        #[arg(long)]
        create_key: String,

        /// Path to payer keypair
        #[arg(long)]
        payer: String,

        /// Optional: time lock delay in seconds
        #[arg(long)]
        time_lock: Option<u32>,

        /// Optional: description/memo
        #[arg(long)]
        memo: Option<String>,
    },
}

fn main() -> eyre::Result<()> {
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
            } => vw::whitelist::add(&cli.url, vote_account, start_epoch, end_epoch, authority)?,
            ValidatorWhitelistAction::Remove {
                vote_account,
                authority,
            } => vw::whitelist::remove(&cli.url, vote_account, authority)?,
            ValidatorWhitelistAction::UpdateStartEpoch {
                vote_account,
                epoch,
                authority,
            } => vw::whitelist::update_start_epoch(&cli.url, vote_account, epoch, authority)?,
            ValidatorWhitelistAction::UpdateEndEpoch {
                vote_account,
                epoch,
                authority,
            } => vw::whitelist::update_end_epoch(&cli.url, vote_account, epoch, authority)?,
            // Authority commands
            ValidatorWhitelistAction::Auth => vw::authority::auth(&cli.url)?,
            ValidatorWhitelistAction::ProposeAuthority {
                new_authority,
                authority,
            } => vw::authority::propose_authority(&cli.url, new_authority, authority)?,
            ValidatorWhitelistAction::AcceptAuthority { authority } => {
                vw::authority::accept_authority(&cli.url, authority)?
            }
            ValidatorWhitelistAction::CancelAuthority { authority } => {
                vw::authority::cancel_authority(&cli.url, authority)?
            }
        },
        Commands::ProgramWhitelist { action } => match action {
            ProgramWhitelistAction::List => pw::whitelist::list(&cli.url)?,
            ProgramWhitelistAction::Add {
                program_authority,
                authority,
            } => pw::whitelist::add(&cli.url, program_authority, authority)?,
            ProgramWhitelistAction::Remove {
                program_authority,
                authority,
            } => pw::whitelist::remove(&cli.url, program_authority, authority)?,
            ProgramWhitelistAction::Auth => pw::authority::auth(&cli.url)?,
            ProgramWhitelistAction::ProposeAuthority {
                new_authority,
                authority,
            } => pw::authority::propose_authority(&cli.url, new_authority, authority)?,
            ProgramWhitelistAction::AcceptAuthority { authority } => {
                pw::authority::accept_authority(&cli.url, authority)?
            }
            ProgramWhitelistAction::CancelAuthority { authority } => {
                pw::authority::cancel_authority(&cli.url, authority)?
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
                payer,
                spill,
            } => loader::upgrade::upgrade_program(
                &cli.url,
                program_id,
                program_so,
                upgrade_authority,
                payer,
                spill,
            )?,
        },
        Commands::Multisig { action } => match action {
            MultisigAction::ProgramConfigInit {
                authority,
                treasury,
                creation_fee,
                initializer,
            } => authority::multisig::program_config_init(
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
            } => authority::multisig::create(
                members,
                threshold,
                create_key,
                payer,
                &cli.url,
                time_lock,
                memo,
            )?,
        },
        Commands::Airdrop { keypair, amount } => {
            airdrop::airdrop::airdrop(&cli.url, keypair, amount)?
        }
    }

    Ok(())
}
