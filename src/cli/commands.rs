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
    /// Monetary policy commands
    #[command(name = "mp")]
    MonetaryPolicy {
        #[command(subcommand)]
        action: MonetaryPolicyAction,
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
        /// Account pubkey to receive the airdrop
        #[arg(long)]
        pubkey: String,
        /// Amount in SOL to airdrop
        #[arg(long, default_value = "1.0")]
        amount: f64,
    },
    /// Transfer SOL from one account to another
    Transfer {
        /// Destination account pubkey
        #[arg(long)]
        destination: String,
        /// Amount in SOL to transfer
        #[arg(long)]
        amount: f64,
        /// Single-sig: path to source keypair (mutually exclusive with --multisig)
        #[arg(long, conflicts_with = "multisig")]
        from: Option<String>,
        /// Multi-sig: multisig PDA address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ValidatorWhitelistAction {
    /// Show validator whitelist account (authority and entries)
    Show,
    /// Add a validator to the whitelist
    Add {
        vote_account: String,
        #[arg(long)]
        start_epoch: Option<u64>,
        #[arg(long)]
        end_epoch: Option<u64>,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Remove a validator from the whitelist
    Remove {
        vote_account: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Update a validator's start epoch
    UpdateStartEpoch {
        vote_account: String,
        #[arg(long)]
        epoch: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Update a validator's end epoch
    UpdateEndEpoch {
        vote_account: String,
        #[arg(long)]
        epoch: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProgramWhitelistAction {
    /// Show program whitelist account (authority + deployers)
    Show,
    /// Whitelist a deployer authority (who can deploy/upgrade programs)
    Add {
        program_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Remove a deployer authority from the whitelist
    Remove {
        program_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
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

        /// Single-sig: path to upgrade authority keypair (mutually exclusive with --vault)
        #[arg(long, conflicts_with = "multisig")]
        upgrade_authority: Option<String>,

        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,

        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,

        /// Payer keypair path
        #[arg(long)]
        payer: String,

        /// Spill account (where excess buffer rent is returned, defaults to payer)
        #[arg(long)]
        spill: Option<String>,
    },
    /// Extend a program's data account to accommodate larger programs
    Extend {
        /// Program ID of the program to extend
        #[arg(long)]
        program_id: String,

        /// Number of additional bytes to add
        #[arg(long)]
        bytes: u32,

        /// Single-sig: path to upgrade authority keypair (mutually exclusive with --multisig)
        #[arg(long, conflicts_with = "multisig")]
        upgrade_authority: Option<String>,

        /// Multi-sig: multisig address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,

        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,

        /// Payer keypair path
        #[arg(long)]
        payer: String,
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

        /// Payer keypair path
        #[arg(long)]
        payer: String,
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
    /// Show multisig vault information (fetches on-chain data)
    Show {
        /// Path to create key keypair used during vault creation (mutually exclusive with --multisig)
        #[arg(long, conflicts_with = "multisig")]
        create_key: Option<String>,

        /// Multisig PDA address (mutually exclusive with --create-key)
        #[arg(long, conflicts_with = "create_key")]
        multisig: Option<String>,
    },
    /// Approve a multisig proposal
    Approve {
        /// Multisig PDA address
        #[arg(long)]
        multisig: String,

        /// Transaction index of the proposal to approve
        #[arg(long)]
        transaction_index: u64,

        /// Path to member keypair who is approving
        #[arg(long)]
        member: String,
    },
    /// Execute an approved multisig proposal
    Execute {
        /// Multisig PDA address
        #[arg(long)]
        multisig: String,

        /// Transaction index of the proposal to execute
        #[arg(long)]
        transaction_index: u64,

        /// Path to member keypair who is executing
        #[arg(long)]
        member: String,
    },
}

#[derive(Subcommand)]
pub enum MonetaryPolicyAction {
    /// Show monetary policy account details
    Show,
    /// Create the monetary policy account
    Create {
        /// Single-sig: path to authority keypair (who will control the policy)
        #[arg(long = "authority", alias = "auth")]
        authority: String,
        /// Path to payer keypair (who pays for account creation)
        #[arg(long)]
        payer: String,
    },
    /// Show authority account
    Auth,
    /// Propose a new authority
    ProposeAuthority {
        new_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Update inflation rate (in basis points)
    UpdateInflationRate {
        /// New inflation rate in basis points (0-2000 bips = 0-20%)
        new_rate_bips: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Update lamports per signature (transaction fee)
    UpdateLamportsPerSignature {
        /// New lamports per signature (1-10,000,000 lamports)
        new_lamports_per_signature: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
    /// Update burn percent
    UpdateBurnPercent {
        /// New burn percent (0-100%)
        new_percent: u8,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(long = "authority", alias = "auth", conflicts_with = "multisig")]
        authority: Option<String>,
        /// Multi-sig: vault address (requires --multisig-authority)
        #[arg(long, requires = "multisig_authority")]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig")]
        multisig_authority: Option<String>,
    },
}
