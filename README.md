# SphereNet Admin CLI

Command-line interface for administering SphereNet validator and program whitelists.

## Overview

SphereNet uses two permissioning systems to control network participation:

- **Validator Whitelist** - Controls which validators can participate in consensus
- **Program Whitelist** - Controls which authorities can deploy/upgrade programs

This CLI provides administrative commands for managing both whitelists.

## Configuration

The CLI connects to SphereNet testnet by default (`https://api.testnet.sphere.net`).

You can override the RPC endpoint using the global `--url` flag:

```bash
# Use local validator
spherenet-admin --url http://localhost:8899 vw list

# Use mainnet (when available)
spherenet-admin --url https://api.mainnet.sphere.net pw list

# Default to testnet
spherenet-admin vw list
```

## Global Flags

- `--url <RPC_URL>` - RPC endpoint to connect to (default: `https://api.testnet.sphere.net`)

## Commands

### Validator Whitelist (`vw`)

Manage which validators can participate in consensus.

#### List Validators

```bash
spherenet-admin vw list
```

Shows all whitelisted validators with their vote accounts, start epochs, and end epochs.

#### Add Validator

**Single-Sig:**
```bash
spherenet-admin vw add <VOTE_ACCOUNT> \
  --start-epoch <EPOCH> \
  --end-epoch <EPOCH> \
  --authority <PATH>
```

**Multi-Sig (creates proposal):**
```bash
spherenet-admin vw add <VOTE_ACCOUNT> \
  --start-epoch <EPOCH> \
  --end-epoch <EPOCH> \
  --multisig <MULTISIG_PDA> \
  --multisig-authority <PATH>
```

**Arguments:**
- `<VOTE_ACCOUNT>` - Validator's vote account public key
- `--start-epoch` - (Optional) Epoch when validator can start voting (default: current epoch)
- `--end-epoch` - (Optional) Epoch when validator's term ends (default: u64::MAX)
- `--authority` - Path to whitelist authority keypair (single-sig)
- `--multisig` - Multisig PDA address (multi-sig, requires `--multisig-authority`)
- `--multisig-authority` - Path to member keypair (multi-sig)

**Example:**
```bash
# Single-sig
spherenet-admin vw add HqzWjPX... \
  --start-epoch 100 \
  --end-epoch 200 \
  --authority ./authority.json

# Multi-sig
spherenet-admin vw add HqzWjPX... \
  --start-epoch 100 \
  --end-epoch 200 \
  --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t \
  --multisig-authority ./member-0.json
```

**Note:** All validator whitelist commands (add, remove, update epochs, propose/accept/cancel authority) support both single-sig and multi-sig modes.

#### Remove Validator

```bash
spherenet-admin vw remove <VOTE_ACCOUNT> --auth <PATH>
```

**Note:** Cannot remove validators during their active term. Update the end epoch first, wait for the epoch boundary, then remove.

#### Update Start Epoch

```bash
spherenet-admin vw update-start-epoch <VOTE_ACCOUNT> \
  --epoch <EPOCH> \
  --auth <PATH>
```

Updates when a validator can start voting.

#### Update End Epoch

```bash
spherenet-admin vw update-end-epoch <VOTE_ACCOUNT> \
  --epoch <EPOCH> \
  --auth <PATH>
```

Updates when a validator's term ends. Must be greater than current epoch.

#### View Authority

```bash
spherenet-admin vw auth
```

Shows current whitelist authority, pending authority (if any), and whitelist account info.

#### Transfer Authority

```bash
# Step 1: Propose new authority
spherenet-admin vw propose-authority <NEW_AUTHORITY_PUBKEY> \
  --auth <CURRENT_AUTHORITY_KEYPAIR>

# Step 2: Accept as new authority
spherenet-admin vw accept-authority \
  --auth <NEW_AUTHORITY_KEYPAIR>

# Or cancel the transfer
spherenet-admin vw cancel-authority \
  --auth <CURRENT_AUTHORITY_KEYPAIR>
```

Two-step authority transfer prevents accidental loss of control.

---

### Program Whitelist (`pw`)

Manage which authorities can deploy/upgrade programs on SphereNet.

#### List Deployer Authorities

```bash
spherenet-admin pw list
```

Shows all authorities that are allowed to deploy programs.

#### Add Deployer Authority

**Single-Sig:**
```bash
spherenet-admin pw add <DEPLOYER_AUTHORITY> --authority <PATH>
```

**Multi-Sig (creates proposal):**
```bash
spherenet-admin pw add <DEPLOYER_AUTHORITY> \
  --multisig <MULTISIG_PDA> \
  --multisig-authority <PATH>
```

**Arguments:**
- `<DEPLOYER_AUTHORITY>` - Public key of the authority that can deploy programs
- `--authority` - Path to whitelist authority keypair (single-sig)
- `--multisig` - Multisig PDA address (multi-sig, requires `--multisig-authority`)
- `--multisig-authority` - Path to member keypair (multi-sig)

**Example:**
```bash
# Single-sig
spherenet-admin pw add 8AydpnGCywwzhVh4ZCUv7HWKodQgCk3Mic33ybGZ7Yjp \
  --authority ./authority.json

# Multi-sig
spherenet-admin pw add 8AydpnGCywwzhVh4ZCUv7HWKodQgCk3Mic33ybGZ7Yjp \
  --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t \
  --multisig-authority ./member-0.json
```

**Notes:**
- The program whitelist controls **WHO** can deploy programs (authorities), not **WHICH** programs can execute
- All program whitelist commands (add, remove, propose/accept/cancel authority) support both single-sig and multi-sig modes

#### Remove Deployer Authority

```bash
spherenet-admin pw remove <DEPLOYER_AUTHORITY> --auth <PATH>
```

Removes an authority's permission to deploy/upgrade programs.

#### View Authority

```bash
spherenet-admin pw auth
```

Shows current whitelist authority, pending authority (if any), and whitelist account info.

#### Transfer Authority

```bash
# Step 1: Propose new authority
spherenet-admin pw propose-authority <NEW_AUTHORITY_PUBKEY> \
  --auth <CURRENT_AUTHORITY_KEYPAIR>

# Step 2: Accept as new authority
spherenet-admin pw accept-authority \
  --auth <NEW_AUTHORITY_KEYPAIR>

# Or cancel the transfer
spherenet-admin pw cancel-authority \
  --auth <CURRENT_AUTHORITY_KEYPAIR>
```

---

### Program Deployment (`program`)

Deploy and upgrade programs on SphereNet with whitelist enforcement.

#### Deploy Program

```bash
spherenet-admin program deploy \
  --program-so <PATH> \
  --program-keypair <PATH> \
  --upgrade-authority <PATH> \
  --payer <PATH>
```

**Arguments:**
- `--program-so` - Path to compiled program binary (.so file)
- `--program-keypair` - Path to program keypair (determines program ID)
- `--upgrade-authority` - Path to upgrade authority keypair (must be whitelisted)
- `--payer` - Path to payer keypair (funds buffer creation)

**Example:**
```bash
spherenet-admin program deploy \
  --program-so ./target/deploy/my_program.so \
  --program-keypair ./target/deploy/my_program-keypair.json \
  --upgrade-authority ./authority.json \
  --payer ~/.config/solana/id.json
```

**Notes:**
- Upgrade authority must be whitelisted via `spherenet-admin pw add` before deployment
- Command validates whitelist before creating buffer to fail fast
- Buffer creation costs ~0.13 SOL rent (returned on successful deploy)

#### Upgrade Program

**Single-Sig:**
```bash
spherenet-admin program upgrade \
  --program-id <PUBKEY> \
  --program-so <PATH> \
  --upgrade-authority <PATH> \
  --payer <PATH> \
  [--spill <PUBKEY>]
```

**Multi-Sig (creates proposal):**
```bash
spherenet-admin program upgrade \
  --program-id <PUBKEY> \
  --program-so <PATH> \
  --multisig <MULTISIG_PDA> \
  --multisig-authority <PATH> \
  --payer <PATH> \
  [--spill <PUBKEY>]
```

**Arguments:**
- `--program-id` - Program ID to upgrade (as pubkey string)
- `--program-so` - Path to new compiled program binary
- `--upgrade-authority` - Path to upgrade authority keypair (single-sig, must be whitelisted)
- `--multisig` - Multisig PDA address (multi-sig, vault must be whitelisted)
- `--multisig-authority` - Path to member keypair (multi-sig, proposes and pays)
- `--payer` - Path to payer keypair (funds buffer creation)
- `--spill` - (Optional) Account to receive excess buffer rent (defaults to payer)

**Example:**
```bash
# Single-sig
spherenet-admin program upgrade \
  --program-id 7vH9zQdPKjLKJNHh8AzPBpVYQBvzRUQoQ7vP7ENSNqzL \
  --program-so ./target/deploy/my_program.so \
  --upgrade-authority ./authority.json \
  --payer ~/.config/solana/id.json

# Multi-sig
spherenet-admin program upgrade \
  --program-id 7vH9zQdPKjLKJNHh8AzPBpVYQBvzRUQoQ7vP7ENSNqzL \
  --program-so ./target/deploy/my_program.so \
  --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t \
  --multisig-authority ./member-0.json \
  --payer ~/.config/solana/id.json
```

**Notes:**
- Checks program capacity before creating buffer (use `solana program extend` if needed)
- Fails with clear error message showing exact extend command if program too small
- Upgrade authority must remain whitelisted (removing authority prevents upgrades)
- For multi-sig: The multisig vault PDA must be whitelisted as a deployer authority

**⚠️ Multisig Program Authority Limitations**

Due to Solana's security model, the BPF Loader Upgradeable program has specific CPI restrictions:

**What Works:**
- ✅ Program upgrades through multisig
- ✅ Program extend through multisig (uses `ExtendProgramChecked` instruction)
- ✅ Transferring upgrade authority **TO** a multisig (one-time setup)

**What Does NOT Work (Currently):**
- ❌ Transferring upgrade authority **FROM** a multisig back to single-sig

**Understanding CPI-Safe "Checked" Variants:**

The BPF Loader has two variants for many operations, following a security pattern:

**Extend Instructions:**
- `ExtendProgram` (unchecked) - ❌ Blocked from CPI for security
- `ExtendProgramChecked` (checked) - ✅ Explicitly allowed for CPI, requires authority signature

**Authority Transfer Instructions:**
- `SetUpgradeAuthority` (unchecked) - ❌ Blocked from CPI for security
- `SetUpgradeAuthorityChecked` (checked) - ✅ Allowed for CPI, but requires BOTH current and new authorities to sign

**Why Multisig → Single-Sig Transfer Doesn't Work:**

While `SetUpgradeAuthorityChecked` IS in the CPI allowlist, it requires both authorities to sign the same transaction:
- Current authority (vault PDA) - Can sign via invoke_signed ✅
- New authority (single-sig keypair) - Would need to be present in the multisig execution context ❌

This creates a coordination problem that Squads' execution model doesn't currently support. However, this could be implemented as an "escape hatch" feature in the future if needed.

**Best Practices:**

1. **Deploy with reasonable `--max-data-len`** for future growth (you can always extend via multisig if needed)
   ```bash
   spherenet-admin program deploy \
     --program-so ./target/deploy/my_program.so \
     --program-keypair ./program-keypair.json \
     --upgrade-authority ./authority.json \
     --payer ./payer.json \
     --max-data-len 100000
   ```

2. **Extend when needed through multisig:**
   ```bash
   spherenet-admin program extend \
     --program-id <PROGRAM_ID> \
     --bytes 10000 \
     --multisig <MULTISIG_PDA> \
     --multisig-authority ./member-0.json \
     --payer ./member-0.json
   ```

3. **Transfer upgrade authority TO multisig using standard Solana CLI:**
   ```bash
   # Single-sig → Multisig (uses unchecked variant, skips new authority signature)
   solana program set-upgrade-authority <PROGRAM_ID> \
     --new-upgrade-authority <VAULT_PDA> \
     --upgrade-authority ./current-authority.json \
     --skip-new-upgrade-authority-signer-check
   ```

   **Note:** The `--skip-new-upgrade-authority-signer-check` flag uses the unchecked `SetUpgradeAuthority` instruction, which only requires the current authority to sign. This is safe because you're explicitly confirming the new authority address.

**Complete Program Lifecycle with Multisig:**
1. Deploy program with single-sig authority
2. Transfer authority to multisig vault PDA (one-time, using solana CLI)
3. Upgrade through multisig governance (spherenet-admin)
4. Extend capacity through multisig governance (spherenet-admin)
5. Continue upgrading as needed

Once authority is transferred to multisig, the program is under full governance control. While there's no built-in way to transfer authority back to single-sig, this is philosophically correct for governance - true decentralization means the multisig retains permanent control.

#### Helper Scripts

Convenience scripts in `scripts/` directory use environment variables:

```bash
# Copy and configure environment
cp scripts/.env.example scripts/.env
# Edit .env with your paths

# Deploy using env vars
./scripts/testnet_program_deploy.sh

# Upgrade using env vars
./scripts/testnet_program_upgrade.sh
```

---

### Multisig Management (`multisig`)

Create and manage Squads v4 multisig vaults for decentralized control.

#### Initialize Program Config (One-time Setup)

```bash
spherenet-admin multisig program-config-init \
  --authority <PUBKEY> \
  --treasury <PUBKEY> \
  --creation-fee <LAMPORTS> \
  --initializer <PATH>
```

**Arguments:**
- `--authority` - Pubkey that will control the program config
- `--treasury` - Pubkey where multisig creation fees are sent
- `--creation-fee` - Fee in lamports for creating a multisig (default: 0)
- `--initializer` - Path to INITIALIZER keypair (hardcoded in program)

#### Create Multisig Vault

```bash
spherenet-admin multisig create \
  --members <PUBKEY>,<PUBKEY>,<PUBKEY> \
  --threshold <NUMBER> \
  --create-key <PATH> \
  --payer <PATH> \
  [--time-lock <SECONDS>] \
  [--memo <TEXT>]
```

**Arguments:**
- `--members` - Comma-separated list of member pubkeys
- `--threshold` - Number of approvals required (e.g., 2 for 2-of-3)
- `--create-key` - Path to create key keypair (unique ID for this multisig)
- `--payer` - Path to payer keypair
- `--time-lock` - (Optional) Delay in seconds before proposals can execute
- `--memo` - (Optional) Description of the multisig

**Example:**
```bash
spherenet-admin multisig create \
  --members 8AydpnGCywwzhVh4ZCUv7HWKodQgCk3Mic33ybGZ7Yjp,HqzWjPX...,5jV5k... \
  --threshold 2 \
  --create-key ./multisig-create-key.json \
  --payer ~/.config/solana/id.json \
  --memo "Treasury multisig 2-of-3"
```

#### Show Multisig Information

```bash
# Using create key
spherenet-admin multisig show --create-key <PATH>

# Using multisig PDA (if you don't have the create key)
spherenet-admin multisig show --multisig <PUBKEY>
```

Displays:
- Multisig PDA address
- Vault PDA address (where funds are held)
- Vault balance
- Threshold and member list
- Current transaction index

#### Approve Proposal

```bash
spherenet-admin multisig approve \
  --multisig <MULTISIG_PDA> \
  --transaction-index <INDEX> \
  --member <PATH>
```

**Arguments:**
- `--multisig` - Multisig PDA address
- `--transaction-index` - Transaction index to approve
- `--member` - Path to member keypair who is approving

#### Execute Proposal

```bash
spherenet-admin multisig execute \
  --multisig <MULTISIG_PDA> \
  --transaction-index <INDEX> \
  --member <PATH>
```

**Arguments:**
- `--multisig` - Multisig PDA address
- `--transaction-index` - Transaction index to execute (must have enough approvals)
- `--member` - Path to member keypair who is executing

---

### Transfer

Transfer SOL between accounts with single-sig or multi-sig support.

#### Single-Sig Transfer

```bash
spherenet-admin transfer \
  --destination <PUBKEY> \
  --amount <SOL> \
  --from <PATH>
```

**Arguments:**
- `--destination` - Destination account pubkey
- `--amount` - Amount in SOL to transfer
- `--from` - Path to source keypair

**Example:**
```bash
spherenet-admin transfer \
  --destination 8Y7NwzMQXD6EpRYMyFBp6Xkxt9wUPREneDktezi53r4s \
  --amount 1.5 \
  --from ~/.config/solana/id.json
```

#### Multi-Sig Transfer (Creates Proposal)

```bash
spherenet-admin transfer \
  --destination <PUBKEY> \
  --amount <SOL> \
  --multisig <MULTISIG_PDA> \
  --multisig-authority <PATH>
```

**Arguments:**
- `--destination` - Destination account pubkey
- `--amount` - Amount in SOL to transfer
- `--multisig` - Multisig PDA address (vault holds the funds)
- `--multisig-authority` - Path to member keypair (proposes and pays for proposal)

**Example:**
```bash
spherenet-admin transfer \
  --destination 8Y7NwzMQXD6EpRYMyFBp6Xkxt9wUPREneDktezi53r4s \
  --amount 0.5 \
  --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t \
  --multisig-authority ./member-0.json
```

Creates a proposal that members can approve and execute.

---

### Airdrop

Request SOL airdrop (testnet only).

```bash
spherenet-admin airdrop \
  --pubkey <PUBKEY> \
  --amount <SOL_AMOUNT>
```

**Arguments:**
- `--pubkey` - Account pubkey to receive the airdrop
- `--amount` - Amount of SOL (default: 1.0)

**Example:**
```bash
spherenet-admin airdrop \
  --pubkey 8Y7NwzMQXD6EpRYMyFBp6Xkxt9wUPREneDktezi53r4s \
  --amount 5.0
```

## Architecture

The CLI uses generated instruction builders from:
- `spherenet-validator-whitelist-client` - For validator whitelist operations
- `spherenet-program-whitelist-client` - For program whitelist operations

Both are generated from their respective interface definitions using Codama/Kinobi.

### Multisig Architecture

The CLI integrates with Squads v4 for multisig functionality. Key architectural concepts:

- **Multisig PDA**: Control structure storing configuration (members, threshold, transaction index)
  - Derived from: `[SEED_PREFIX, SEED_MULTISIG, create_key]`

- **Vault PDA**: Where funds are actually held and that signs transactions
  - Derived from: `[SEED_PREFIX, multisig_pda, SEED_VAULT, vault_index]`

- **Authority Abstraction**: `Authority` enum provides unified interface for single-sig and multi-sig execution
  - Single-sig: Executes instructions directly
  - Multi-sig: Creates proposals that require member approvals

- **SmallVec Serialization**: Custom implementation for Squads' variable-length vector format
  - `SmallVec<u8, T>`: 1-byte length prefix
  - `SmallVec<u16, T>`: 2-byte length prefix
  - Required for proper transaction message serialization

## Network Information

**Testnet:**
- RPC: `https://api.testnet.sphere.net`
- Validator Whitelist Program: `wLpnFMEvuP6hPE84AGrsmNr2Bo2uk69MC4kKWtrWHBN`
- Program Whitelist Program: `PwLzPtX2e5PwNFQTrEY5DkkmRQ1t1x5vWcGzTnMgibR`
- Squads v4 Program: `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`

### Project Structure

```
src/
├── main.rs           # Entry point
├── cli/
│   ├── mod.rs        # CLI module exports
│   ├── commands.rs   # CLAP command definitions
│   ├── run.rs        # Command routing
│   └── authority.rs  # Authority abstraction (single-sig & multi-sig)
├── vw/
│   ├── mod.rs        # Validator whitelist module exports
│   ├── whitelist.rs  # Validator whitelist commands
│   └── authority.rs  # Validator authority management
├── pw/
│   ├── mod.rs        # Program whitelist module exports
│   ├── whitelist.rs  # Program whitelist commands
│   └── authority.rs  # Program authority management
├── loader/
│   ├── mod.rs        # Loader utilities (write_buffer helper)
│   ├── deploy.rs     # Program deployment implementation
│   └── upgrade.rs    # Program upgrade implementation
├── squads/
│   ├── mod.rs        # Squads v4 integration module exports
│   ├── types.rs      # Squads types, SmallVec, PDAs
│   ├── instructions.rs  # Instruction builders
│   └── commands.rs   # User-facing multisig commands
└── utils/
    ├── mod.rs        # Utility module exports
    ├── airdrop.rs    # Airdrop implementation
    └── transfer.rs   # Transfer implementation (single & multi-sig)

scripts/
├── .env.example                   # Environment variable template
├── testnet_program_deploy.sh     # Deploy script with env vars
├── testnet_program_upgrade.sh    # Upgrade script with env vars
└── testnet_health_check.sh       # Network health monitoring
```
