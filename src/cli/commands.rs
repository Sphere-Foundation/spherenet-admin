//! CLI command definitions
//!
//! CLAP structs and enums defining the CLI interface.
//!
//! Convention for arg value placeholders: `*_KEYPAIR` = path to a keypair file
//! (signs), `*_PUBKEY` = a public key, `*_CREATE_KEY` = a multisig's create-key
//! (pubkey), `*_SO` = a program `.so` file. This makes `--help`/error output say
//! whether each argument is a keypair or a pubkey.

use crate::cli::output::OutputMode;
use clap::{Parser, Subcommand};

/// Default RPC_URL
pub const RPC_URL: &str = "https://api.test.sphere.net";

/// ASCII banner shown at the top of `--help` — the same Small Slant "SphereNet"
/// art as the read-API banner in `server::run`, tagged `admin`.
pub const BANNER_ART: &str = concat!(
    "\n",
    r"   ____     __               _  __    __
  / __/__  / /  ___ _______ / |/ /__ / /_
 _\ \/ _ \/ _ \/ -_) __/ -_)    / -_) __/
/___/ .__/_//_/\__/_/  \__/_/|_/\__/\__/
   /_/                         a d m i n",
);

#[derive(Parser)]
#[command(name = "spherenet-admin", about = BANNER_ART, version)]
pub struct Cli {
    /// RPC URL to connect to
    #[arg(long, global = true, default_value = RPC_URL, value_name = "URL")]
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
        #[arg(value_name = "PUBKEY")]
        pubkey: String,
    },
    /// Show the current epoch
    Epoch,
    /// Serve command output as a read-only JSON HTTP API
    Server {
        /// Port to listen on
        #[arg(long, default_value = "8080", value_name = "PORT")]
        port: u16,
    },
    /// Request an airdrop for an account
    Airdrop {
        /// Account pubkey to receive the airdrop
        #[arg(long, value_name = "PUBKEY")]
        pubkey: String,
        /// Amount in SPHR to airdrop
        #[arg(long, default_value = "1.0", value_name = "SPHR")]
        amount: f64,
    },
    /// Transfer SPHR from one account to another
    Transfer {
        /// Destination account pubkey (mutually exclusive with --to-multisig)
        #[arg(
            long,
            conflicts_with = "to_multisig",
            value_name = "DESTINATION_PUBKEY"
        )]
        to: Option<String>,
        /// Destination multisig by create-key — funds go to its vault, validated
        /// (mutually exclusive with --to)
        #[arg(long, conflicts_with = "to", value_name = "MULTISIG_CREATE_KEY")]
        to_multisig: Option<String>,
        /// Amount in SPHR to transfer
        #[arg(long, value_name = "SPHR")]
        amount: f64,
        /// Single-sig: path to source keypair (mutually exclusive with --multisig)
        #[arg(long, conflicts_with = "multisig", value_name = "FROM_KEYPAIR")]
        from: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ValidatorWhitelistAction {
    /// Show validator whitelist account (authority and entries)
    Show,
    /// Request a validator whitelist entry (step 1 of 2 — creates a Pending
    /// entry the authority must approve). The vote account co-signs to prove
    /// control, so this is single-sig only (multisig cannot co-sign).
    Request {
        /// Path to the validator vote-account keypair (co-signs to prove control)
        #[arg(value_name = "VOTE_ACCOUNT_KEYPAIR")]
        vote_account_keypair: String,
        #[arg(long, value_name = "EPOCH")]
        start_epoch: Option<u64>,
        #[arg(long, value_name = "EPOCH")]
        end_epoch: Option<u64>,
        /// Single-sig: path to authority/payer keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Approve a pending validator whitelist entry (step 2 of 2 — authority action)
    Approve {
        #[arg(value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        #[arg(long, value_name = "EPOCH")]
        start_epoch: Option<u64>,
        #[arg(long, value_name = "EPOCH")]
        end_epoch: Option<u64>,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Reject a pending validator whitelist entry (authority action)
    Reject {
        #[arg(value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Remove a validator from the whitelist
    Remove {
        #[arg(value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Update a validator's start epoch
    UpdateStartEpoch {
        #[arg(value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        #[arg(long, value_name = "EPOCH")]
        epoch: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Update a validator's end epoch
    UpdateEndEpoch {
        #[arg(value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        #[arg(long, value_name = "EPOCH")]
        epoch: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Propose a new authority
    ProposeAuthority {
        /// New authority pubkey (mutually exclusive with --new-multisig)
        #[arg(
            long,
            conflicts_with = "new_multisig",
            value_name = "NEW_AUTHORITY_PUBKEY"
        )]
        new_authority: Option<String>,
        /// New authority is a multisig, by create-key — resolves to its vault
        /// (mutually exclusive with --new-authority)
        #[arg(
            long,
            conflicts_with = "new_authority",
            value_name = "NEW_MULTISIG_CREATE_KEY"
        )]
        new_multisig: Option<String>,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProgramWhitelistAction {
    /// Show program whitelist account (authority + deployers)
    Show,
    /// Request a deployer whitelist entry (step 1 of 2 — creates a Pending entry
    /// the authority must approve). The deploy authority co-signs to prove
    /// control, so this is single-sig only (multisig cannot co-sign).
    Request {
        /// Path to the deploy-authority keypair (co-signs to prove control)
        #[arg(value_name = "DEPLOY_AUTHORITY_KEYPAIR")]
        deploy_authority_keypair: String,
        /// Single-sig: path to authority/payer keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Approve a pending deployer whitelist entry (step 2 of 2 — authority action)
    Approve {
        #[arg(value_name = "DEPLOYER_PUBKEY")]
        deploy_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Reject a pending deployer whitelist entry (authority action)
    Reject {
        #[arg(value_name = "DEPLOYER_PUBKEY")]
        deploy_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Remove a deployer authority from the whitelist
    Remove {
        #[arg(value_name = "DEPLOYER_PUBKEY")]
        deploy_authority: String,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Propose a new authority
    ProposeAuthority {
        /// New authority pubkey (mutually exclusive with --new-multisig)
        #[arg(
            long,
            conflicts_with = "new_multisig",
            value_name = "NEW_AUTHORITY_PUBKEY"
        )]
        new_authority: Option<String>,
        /// New authority is a multisig, by create-key — resolves to its vault
        /// (mutually exclusive with --new-authority)
        #[arg(
            long,
            conflicts_with = "new_authority",
            value_name = "NEW_MULTISIG_CREATE_KEY"
        )]
        new_multisig: Option<String>,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProgramAction {
    /// Deploy a program to SphereNet
    Deploy {
        /// Path to the program .so file
        #[arg(long, value_name = "PROGRAM_SO")]
        program_so: String,

        /// Path to the program keypair (the program ID is derived from this)
        #[arg(long, value_name = "PROGRAM_KEYPAIR")]
        program_keypair: String,

        /// Path to upgrade authority keypair (must sign deployment)
        #[arg(long, value_name = "UPGRADE_AUTHORITY_KEYPAIR")]
        upgrade_authority: String,

        /// Payer keypair path (defaults to solana config default keypair)
        #[arg(long, value_name = "PAYER_KEYPAIR")]
        payer: String,

        /// Maximum program data length (optional, defaults to program size)
        #[arg(long, value_name = "BYTES")]
        max_data_len: Option<usize>,
    },
    /// Upgrade an existing program on SphereNet
    Upgrade {
        /// Program ID of the existing program to upgrade
        #[arg(long, value_name = "PROGRAM_ID")]
        program_id: String,

        /// Path to the new program .so file
        #[arg(long, value_name = "PROGRAM_SO")]
        program_so: String,

        /// Single-sig: path to upgrade authority keypair (mutually exclusive with --multisig)
        #[arg(
            long,
            conflicts_with = "multisig",
            value_name = "UPGRADE_AUTHORITY_KEYPAIR"
        )]
        upgrade_authority: Option<String>,

        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,

        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,

        /// Payer keypair path
        #[arg(long, value_name = "PAYER_KEYPAIR")]
        payer: String,

        /// Spill account (where excess buffer rent is returned, defaults to payer)
        #[arg(long, value_name = "SPILL_PUBKEY")]
        spill: Option<String>,
    },
    /// Extend a program's data account to accommodate larger programs
    Extend {
        /// Program ID of the program to extend
        #[arg(long, value_name = "PROGRAM_ID")]
        program_id: String,

        /// Number of additional bytes to add
        #[arg(long, value_name = "BYTES")]
        bytes: u32,

        /// Single-sig: path to upgrade authority keypair (mutually exclusive with --multisig)
        #[arg(
            long,
            conflicts_with = "multisig",
            value_name = "UPGRADE_AUTHORITY_KEYPAIR"
        )]
        upgrade_authority: Option<String>,

        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,

        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,

        /// Payer keypair path
        #[arg(long, value_name = "PAYER_KEYPAIR")]
        payer: String,
    },
    /// Transfer a program's upgrade authority (or renounce it with --final)
    SetUpgradeAuthority {
        /// Program ID whose upgrade authority to transfer
        #[arg(long, value_name = "PROGRAM_ID")]
        program_id: String,

        /// New authority pubkey (mutually exclusive with --new-multisig)
        #[arg(
            long,
            conflicts_with = "new_multisig",
            value_name = "NEW_AUTHORITY_PUBKEY"
        )]
        new_authority: Option<String>,
        /// New authority is a multisig, by create-key — resolves to its vault
        /// (mutually exclusive with --new-authority)
        #[arg(
            long,
            conflicts_with = "new_authority",
            value_name = "NEW_MULTISIG_CREATE_KEY"
        )]
        new_multisig: Option<String>,
        /// Renounce upgradeability — set the authority to none (program becomes
        /// permanently immutable). Mutually exclusive with --new-authority/--new-multisig.
        #[arg(long = "final", conflicts_with_all = ["new_authority", "new_multisig"])]
        make_final: bool,

        /// Single-sig: path to the current upgrade authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum MultisigAction {
    /// Create a new multisig vault
    Create {
        /// Comma-separated list of member pubkeys
        #[arg(long, value_name = "MEMBER_PUBKEYS")]
        members: String,

        /// Number of approvals required (e.g., 2 for 2-of-3)
        #[arg(long, value_name = "THRESHOLD")]
        threshold: u16,

        /// Path to create-key keypair (unique ID for this multisig; signs creation)
        #[arg(long, value_name = "CREATE_KEY_KEYPAIR")]
        create_key: String,

        /// Path to payer keypair
        #[arg(long, value_name = "PAYER_KEYPAIR")]
        payer: String,

        /// Optional: time lock delay in seconds
        #[arg(long, value_name = "SECONDS")]
        time_lock: Option<u32>,

        /// Optional: description/memo
        #[arg(long, value_name = "MEMO")]
        memo: Option<String>,
    },
    /// Show multisig vault information (fetches on-chain data)
    Show {
        /// The multisig, identified by its create-key (pubkey)
        #[arg(long = "multisig", value_name = "MULTISIG_CREATE_KEY")]
        create_key: String,
    },
    /// Approve a multisig proposal
    Approve {
        /// The multisig, identified by its create-key (pubkey)
        #[arg(long = "multisig", value_name = "MULTISIG_CREATE_KEY")]
        create_key: String,

        /// Transaction index of the proposal to approve
        #[arg(long, value_name = "INDEX")]
        transaction_index: u64,

        /// Path to member keypair who is approving
        #[arg(long, value_name = "MEMBER_KEYPAIR")]
        member: String,
    },
    /// Execute an approved multisig proposal
    Execute {
        /// The multisig, identified by its create-key (pubkey)
        #[arg(long = "multisig", value_name = "MULTISIG_CREATE_KEY")]
        create_key: String,

        /// Transaction index of the proposal to execute
        #[arg(long, value_name = "INDEX")]
        transaction_index: u64,

        /// Path to member keypair who is executing
        #[arg(long, value_name = "MEMBER_KEYPAIR")]
        member: String,
    },
}

#[derive(Subcommand)]
pub enum MonetaryPolicyAction {
    /// Show monetary policy account details
    Show,
    /// Propose a new authority
    ProposeAuthority {
        /// New authority pubkey (mutually exclusive with --new-multisig)
        #[arg(
            long,
            conflicts_with = "new_multisig",
            value_name = "NEW_AUTHORITY_PUBKEY"
        )]
        new_authority: Option<String>,
        /// New authority is a multisig, by create-key — resolves to its vault
        /// (mutually exclusive with --new-authority)
        #[arg(
            long,
            conflicts_with = "new_authority",
            value_name = "NEW_MULTISIG_CREATE_KEY"
        )]
        new_multisig: Option<String>,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Accept pending authority transfer
    AcceptAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Cancel pending authority transfer
    CancelAuthority {
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Update inflation rate (in basis points)
    UpdateInflationRate {
        /// New inflation rate in basis points (0-2000 bips = 0-20%)
        #[arg(value_name = "BIPS")]
        new_rate_bips: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Update lamports per signature (transaction fee)
    UpdateLamportsPerSignature {
        /// New lamports per signature (1-10,000,000 lamports)
        #[arg(value_name = "LAMPORTS")]
        new_lamports_per_signature: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Update burn percent
    UpdateBurnPercent {
        /// New burn percent (0-100%)
        #[arg(value_name = "PERCENT")]
        new_percent: u8,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
    /// Update VAT lamports per epoch (Vote Admission Ticket price for Alpenglow voting)
    UpdateVatLamportsPerEpoch {
        /// New VAT lamports per epoch (0-100,000,000,000 lamports)
        #[arg(value_name = "LAMPORTS")]
        new_vat_lamports: u64,
        /// Single-sig: path to authority keypair (mutually exclusive with --multisig)
        #[arg(
            long = "authority",
            alias = "auth",
            conflicts_with = "multisig",
            value_name = "AUTHORITY_KEYPAIR"
        )]
        authority: Option<String>,
        /// Multi-sig: the multisig's create-key (pubkey) (requires --multisig-authority)
        #[arg(
            long,
            requires = "multisig_authority",
            value_name = "MULTISIG_CREATE_KEY"
        )]
        multisig: Option<String>,
        /// Multi-sig: path to signer keypair that pays for proposal creation
        #[arg(long, requires = "multisig", value_name = "MEMBER_KEYPAIR")]
        multisig_authority: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum VoteAction {
    /// Show a vote account's identity, authorities, commission, and voting state
    Show {
        /// Vote account pubkey
        #[arg(value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
    },
    /// Create and initialize a vote account
    Create {
        /// Path to the new vote account keypair (signs its own creation)
        #[arg(long, value_name = "VOTE_ACCOUNT_KEYPAIR")]
        vote_account: String,
        /// Path to the validator identity (node) keypair (signs initialization)
        #[arg(long, value_name = "IDENTITY_KEYPAIR")]
        identity: String,
        /// Path to the keypair authorized to submit votes. Optional — defaults to
        /// the identity keypair (identity == voter). A keypair (not a pubkey)
        /// because on the default V1 path it must sign the BLS-append, and the BLS
        /// voter key is derived from it.
        #[arg(long, value_name = "AUTHORIZED_VOTER_KEYPAIR")]
        authorized_voter: Option<String>,
        /// Pubkey authorized to withdraw from the vote account. Optional —
        /// defaults to the authorized voter (which itself defaults to identity),
        /// tripping a hot-key warning since that key can drain the account; prefer
        /// a distinct cold key.
        #[arg(long, value_name = "AUTHORIZED_WITHDRAWER_PUBKEY")]
        authorized_withdrawer: Option<String>,
        /// Inflation-rewards commission percentage (0-100)
        #[arg(long, default_value = "100", value_name = "PERCENT")]
        commission: u8,
        /// Path to keypair that funds the vote account's rent-exempt reserve.
        /// Optional — defaults to the identity keypair.
        #[arg(long, value_name = "FUNDER_KEYPAIR")]
        from: Option<String>,
        /// Path to keypair that pays transaction fees. Optional — defaults to the
        /// identity keypair.
        #[arg(long, alias = "fee-payer", value_name = "PAYER_KEYPAIR")]
        payer: Option<String>,
        /// Initialize with the V2 instruction (VoteInitV2), setting the BLS key
        /// at creation. Requires the vote-account-initialize-v2 feature
        /// (SIMD-0464) to be active on the network — it is rejected otherwise.
        /// The default (V1) creates with the legacy VoteInit and appends the BLS
        /// key in the same transaction via authorize_checked.
        #[arg(long)]
        vote_init_v2: bool,
    },
    /// Withdraw lamports from a vote account (signed by the withdraw authority)
    Withdraw {
        /// Vote account pubkey
        #[arg(long, value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        /// Destination pubkey that receives the withdrawn SPHR
        #[arg(long, value_name = "DESTINATION_PUBKEY")]
        destination: String,
        /// Amount of SPHR to withdraw (mutually exclusive with --all)
        #[arg(long, conflicts_with = "all", value_name = "SPHR")]
        amount: Option<f64>,
        /// Withdraw the entire balance and close the account
        #[arg(long)]
        all: bool,
        /// Path to the withdraw authority keypair; signs the withdrawal
        #[arg(long, value_name = "WITHDRAW_AUTHORITY_KEYPAIR")]
        withdraw_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer", value_name = "PAYER_KEYPAIR")]
        payer: String,
    },
}

#[derive(Subcommand)]
pub enum StakeAction {
    /// Show a stake account's authorities, lockup, and delegation state
    Show {
        /// Stake account pubkey
        #[arg(value_name = "STAKE_ACCOUNT_PUBKEY")]
        stake_account: String,
    },
    /// Create and initialize a stake account (does not delegate)
    Create {
        /// Path to the new stake account keypair (signs its own creation)
        #[arg(long, value_name = "STAKE_ACCOUNT_KEYPAIR")]
        stake_account: String,
        /// Amount of SPHR to deposit (total; delegatable = amount − rent reserve)
        #[arg(long, value_name = "SPHR")]
        amount: f64,
        /// Pubkey set as the stake authority (staker)
        #[arg(long, value_name = "STAKE_AUTHORITY_PUBKEY")]
        stake_authority: String,
        /// Pubkey set as the withdraw authority
        #[arg(long, value_name = "WITHDRAW_AUTHORITY_PUBKEY")]
        withdraw_authority: String,
        /// Path to keypair that funds the deposited SPHR
        #[arg(long, value_name = "FUNDER_KEYPAIR")]
        from: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer", value_name = "PAYER_KEYPAIR")]
        payer: String,
    },
    /// Delegate an existing stake account to a vote account
    Delegate {
        /// Stake account pubkey (must be initialized, not already delegated)
        #[arg(long, value_name = "STAKE_ACCOUNT_PUBKEY")]
        stake_account: String,
        /// Vote account pubkey to delegate to (must be whitelisted)
        #[arg(long, value_name = "VOTE_ACCOUNT_PUBKEY")]
        vote_account: String,
        /// Path to the stake authority (staker) keypair; signs the delegation
        #[arg(long, value_name = "STAKE_AUTHORITY_KEYPAIR")]
        stake_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer", value_name = "PAYER_KEYPAIR")]
        payer: String,
    },
    /// Deactivate a delegated stake account (begins cooldown)
    Deactivate {
        /// Stake account pubkey (must be delegated)
        #[arg(long, value_name = "STAKE_ACCOUNT_PUBKEY")]
        stake_account: String,
        /// Path to the stake authority (staker) keypair; signs the deactivation
        #[arg(long, value_name = "STAKE_AUTHORITY_KEYPAIR")]
        stake_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer", value_name = "PAYER_KEYPAIR")]
        payer: String,
    },
    /// Withdraw lamports from a stake account (use --all to drain & close)
    Withdraw {
        /// Stake account pubkey
        #[arg(long, value_name = "STAKE_ACCOUNT_PUBKEY")]
        stake_account: String,
        /// Destination pubkey that receives the withdrawn SPHR
        #[arg(long, value_name = "DESTINATION_PUBKEY")]
        destination: String,
        /// Amount of SPHR to withdraw (mutually exclusive with --all)
        #[arg(long, conflicts_with = "all", value_name = "SPHR")]
        amount: Option<f64>,
        /// Withdraw the entire balance and close the account
        #[arg(long)]
        all: bool,
        /// Path to the withdraw authority keypair; signs the withdrawal
        #[arg(long, value_name = "WITHDRAW_AUTHORITY_KEYPAIR")]
        withdraw_authority: String,
        /// Path to keypair that pays transaction fees
        #[arg(long, alias = "fee-payer", value_name = "PAYER_KEYPAIR")]
        payer: String,
    },
}
