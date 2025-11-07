//! CLI command definitions
//!
//! CLAP structs and enums defining the CLI interface.

use clap::{Parser, Subcommand};

/// Default RPC_URL
pub const RPC_URL: &str = "https://api.testnet.sphere.net";

#[derive(Parser)]
#[command(name = "spherenet-admin")]
#[command(about = "SphereNet administration CLI", long_about = None)]
pub struct Cli {
    /// RPC URL to connect to
    #[arg(long, global = true, default_value = RPC_URL)]
    pub url: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
pub enum ValidatorWhitelistAction {
    /// List all whitelisted validators
    List,
    /// Add a validator to the whitelist
    Add {
        vote_account: String,
        #[arg(long)]
        start_epoch: Option<u64>,
        #[arg(long)]
        end_epoch: Option<u64>,
        /// Single-sig: path to authority keypair (mutually exclusive with --vault)
        #[arg(long = "authority", alias = "auth", conflicts_with = "vault")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        vault: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "vault")]
        multisig_authority: Option<String>,
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
pub enum ProgramWhitelistAction {
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
pub enum ProgramAction {
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
pub enum MultisigAction {
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
