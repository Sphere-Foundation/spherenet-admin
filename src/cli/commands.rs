//! CLI command definitions
//!
//! CLAP structs and enums defining the CLI interface.

use crate::cli::output::OutputMode;
use clap::{Parser, Subcommand};

/// Default RPC_URL
pub const RPC_URL: &str = "https://api.test.sphere.net";

#[derive(Parser)]
#[command(name = "spherenet-admin")]
#[command(about = "SphereNet administration CLI", long_about = None)]
pub struct Cli {
    /// RPC URL to connect to
    #[arg(long, global = true, default_value = RPC_URL)]
    pub url: String,

    /// Output format: text (human-readable) or json (machine-readable)
    #[arg(long, global = true, value_enum, default_value_t = OutputMode::Text)]
    pub output: OutputMode,

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
    /// Vote account commands
    Vote {
        #[command(subcommand)]
        action: VoteAction,
    },
    /// Stake account commands
    Stake {
        #[command(subcommand)]
        action: StakeAction,
    },
    /// Show the native (SPHR) balance of an account
    Balance {
        /// Account pubkey
        pubkey: String,
    },
    /// Show the current epoch
    Epoch,
    /// Serve command output as a read-only JSON HTTP API
    Server {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,
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
    /// Update VAT lamports per epoch (Vote Admission Ticket price for Alpenglow voting)
    UpdateVatLamportsPerEpoch {
        /// New VAT lamports per epoch (0-100,000,000,000 lamports)
        new_vat_lamports: u64,
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
pub enum VoteAction {
    /// Show a vote account's identity, authorities, commission, and voting state
    Show {
        /// Vote account pubkey
        vote_account: String,
    },
    /// Create and initialize a vote account
    Create {
        /// Path to the new vote account keypair (signs its own creation)
        #[arg(long)]
        vote_account: String,
        /// Path to the validator identity (node) keypair (signs initialization)
        #[arg(long)]
        identity: String,
        /// Pubkey authorized to submit votes
        #[arg(long)]
        authorized_voter: String,
        /// Pubkey authorized to withdraw from the vote account
        #[arg(long)]
        authorized_withdrawer: String,
        /// Inflation-rewards commission percentage (0-100)
        #[arg(long, default_value = "100")]
        commission: u8,
        /// Path to keypair that funds the vote account's rent-exempt reserve
        #[arg(long)]
        from: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer")]
        payer: String,
    },
}

#[derive(Subcommand)]
pub enum StakeAction {
    /// Show a stake account's authorities, lockup, and delegation state
    Show {
        /// Stake account pubkey
        stake_account: String,
    },
    /// Create and initialize a stake account (does not delegate)
    Create {
        /// Path to the new stake account keypair (signs its own creation)
        #[arg(long)]
        stake_account: String,
        /// Amount of SPHR to deposit (total; delegatable = amount − rent reserve)
        #[arg(long)]
        amount: f64,
        /// Pubkey set as the stake authority (staker)
        #[arg(long)]
        stake_authority: String,
        /// Pubkey set as the withdraw authority
        #[arg(long)]
        withdraw_authority: String,
        /// Path to keypair that funds the deposited SPHR
        #[arg(long)]
        from: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer")]
        payer: String,
    },
    /// Delegate an existing stake account to a vote account
    Delegate {
        /// Stake account pubkey (must be initialized, not already delegated)
        #[arg(long)]
        stake_account: String,
        /// Vote account pubkey to delegate to (must be whitelisted)
        #[arg(long)]
        vote_account: String,
        /// Path to the stake authority (staker) keypair; signs the delegation
        #[arg(long)]
        stake_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer")]
        payer: String,
    },
    /// Deactivate a delegated stake account (begins cooldown)
    Deactivate {
        /// Stake account pubkey (must be delegated)
        #[arg(long)]
        stake_account: String,
        /// Path to the stake authority (staker) keypair; signs the deactivation
        #[arg(long)]
        stake_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer")]
        payer: String,
    },
    /// Withdraw lamports from a stake account (use --all to drain & close)
    Withdraw {
        /// Stake account pubkey
        #[arg(long)]
        stake_account: String,
        /// Destination pubkey that receives the withdrawn SPHR
        #[arg(long)]
        destination: String,
        /// Amount of SPHR to withdraw (mutually exclusive with --all)
        #[arg(long, conflicts_with = "all")]
        amount: Option<f64>,
        /// Withdraw the entire balance and close the account
        #[arg(long)]
        all: bool,
        /// Path to the withdraw authority keypair; signs the withdrawal
        #[arg(long)]
        withdraw_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer")]
        payer: String,
    },
}
