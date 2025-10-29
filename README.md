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

```bash
spherenet-admin vw add <VOTE_ACCOUNT> \
  --start-epoch <EPOCH> \
  --end-epoch <EPOCH> \
  --auth <PATH>
```

**Arguments:**
- `<VOTE_ACCOUNT>` - Validator's vote account public key
- `--start-epoch` - (Optional) Epoch when validator can start voting (default: current epoch)
- `--end-epoch` - (Optional) Epoch when validator's term ends (default: u64::MAX)
- `--auth` - Path to whitelist authority keypair (alias: `--authority`)

**Example:**
```bash
spherenet-admin vw add HqzWjPX... \
  --start-epoch 100 \
  --end-epoch 200 \
  --auth ./authority.json
```

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

```bash
spherenet-admin pw add <DEPLOYER_AUTHORITY> --auth <PATH>
```

**Arguments:**
- `<DEPLOYER_AUTHORITY>` - Public key of the authority that can deploy programs
- `--auth` - Path to whitelist authority keypair (alias: `--authority`)

**Example:**
```bash
spherenet-admin pw add 8AydpnGCywwzhVh4ZCUv7HWKodQgCk3Mic33ybGZ7Yjp \
  --auth ./authority.json
```

**Note:** The program whitelist controls **WHO** can deploy programs (authorities), not **WHICH** programs can execute.

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

### Airdrop

Request SOL airdrop (testnet only).

```bash
spherenet-admin airdrop \
  --keypair <PATH> \
  --amount <SOL_AMOUNT>
```

**Arguments:**
- `--keypair` - Path to keypair receiving the airdrop
- `--amount` - Amount of SOL (default: 1.0)

**Example:**
```bash
spherenet-admin airdrop --keypair ./test-wallet.json --amount 5.0
```

## Architecture

The CLI uses generated instruction builders from:
- `spherenet-validator-whitelist-client` - For validator whitelist operations
- `spherenet-program-whitelist-client` - For program whitelist operations

Both are generated from their respective interface definitions using Codama/Kinobi.

## Network Information

**Testnet:**
- RPC: `https://api.testnet.sphere.net`
- Validator Whitelist Program: `wLpnFMEvuP6hPE84AGrsmNr2Bo2uk69MC4kKWtrWHBN`
- Program Whitelist Program: `PwLzPtX2e5PwNFQTrEY5DkkmRQ1t1x5vWcGzTnMgibR`

### Project Structure

```
src/
├── main.rs       # CLI argument parsing and command routing
├── consts.rs     # Network configuration (RPC URL, program IDs)
├── airdrop.rs    # Airdrop functionality
├── vw/
│   ├── mod.rs        # Validator whitelist module exports
│   ├── whitelist.rs  # Validator whitelist commands
│   └── authority.rs  # Validator authority management
└── pw/
    ├── mod.rs        # Program whitelist module exports
    ├── whitelist.rs  # Program whitelist commands
    └── authority.rs  # Program authority management
```
