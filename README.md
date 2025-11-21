# SphereNet Admin CLI

Command-line interface for administering SphereNet validator and program whitelists.

## Overview

SphereNet administration tool for governance and network management:

**Whitelist Management:**
- **Validator Whitelist** - Control which validators participate in consensus
- **Program Whitelist** - Control which authorities can deploy/upgrade programs

**Program Operations:**
- Deploy, upgrade, and extend programs with whitelist enforcement
- Full lifecycle management with single-sig or multisig governance

**Authority Abstraction:**
- Unified interface for immediate execution (single-sig) or governance (multisig)
- Powered by `spherenet-authority` crate with Squads V4 integration
- Create proposals, gather approvals, execute with any member

## Configuration

Connects to SphereNet testnet by default. Override with `--url <RPC_URL>` flag.

```bash
spherenet-admin --url http://localhost:8899 vw list  # Local validator
spherenet-admin vw list                               # Testnet (default)
```

## Commands

All commands support both single-sig (immediate execution) and multi-sig (proposal creation) modes.

### Validator Whitelist (`vw`)

Manage which validators can participate in consensus.

**Example: Add Validator**

```bash
# Single-sig (executes immediately)
spherenet-admin vw add <VOTE_ACCOUNT> \
  --start-epoch 100 \
  --end-epoch 200 \
  --authority ./authority.json

# Multi-sig (creates proposal for approval)
spherenet-admin vw add <VOTE_ACCOUNT> \
  --start-epoch 100 \
  --end-epoch 200 \
  --multisig <MULTISIG_PDA> \
  --multisig-authority ./member.json
```

**Other Commands:**
- `vw list` - Show all whitelisted validators
- `vw remove <VOTE_ACCOUNT>` - Remove validator from whitelist
- `vw update-start-epoch <VOTE_ACCOUNT> --epoch <N>` - Update when validator can start
- `vw update-end-epoch <VOTE_ACCOUNT> --epoch <N>` - Update when validator term ends
- `vw auth` - View current whitelist authority
- `vw propose-authority <NEW_AUTHORITY>` - Initiate authority transfer
- `vw accept-authority` - Accept pending authority transfer
- `vw cancel-authority` - Cancel pending authority transfer

---

### Program Whitelist (`pw`)

Manage which authorities can deploy/upgrade programs. Controls **WHO** can deploy, not **WHICH** programs can execute.

**Example: Add Deployer Authority**

```bash
# Single-sig
spherenet-admin pw add <DEPLOYER_PUBKEY> --authority ./authority.json

# Multi-sig
spherenet-admin pw add <DEPLOYER_PUBKEY> \
  --multisig <MULTISIG_PDA> \
  --multisig-authority ./member.json
```

**Other Commands:**
- `pw list` - Show all whitelisted deployer authorities
- `pw remove <DEPLOYER>` - Remove deployer authority
- `pw auth` - View current whitelist authority
- `pw propose-authority <NEW_AUTHORITY>` - Initiate authority transfer
- `pw accept-authority` - Accept pending authority transfer
- `pw cancel-authority` - Cancel pending authority transfer

---

### Program Deployment (`program`)

Deploy and upgrade programs with whitelist enforcement. Upgrade authority must be whitelisted before deployment.

**Example: Deploy Program**

```bash
spherenet-admin program deploy \
  --program-so ./target/deploy/my_program.so \
  --program-keypair ./target/deploy/my_program-keypair.json \
  --upgrade-authority ./authority.json \
  --payer ~/.config/solana/id.json \
  --max-data-len 100000  # Optional, recommended for future growth
```

**Example: Upgrade Program (Multi-sig)**

```bash
# Create upgrade proposal
spherenet-admin program upgrade \
  --program-id <PROGRAM_ID> \
  --program-so ./target/deploy/my_program.so \
  --multisig <MULTISIG_PDA> \
  --multisig-authority ./member.json \
  --payer ./member.json

# Then: approve with threshold members, execute by any member
```

**Other Commands:**
- `program extend --program-id <ID> --bytes <N>` - Extend program capacity (works with multisig!)

**Multisig Upgrade Authority Lifecycle:**

1. Deploy with single-sig authority (this tool)
2. Transfer authority to multisig: `solana program set-upgrade-authority <PROGRAM> --new-upgrade-authority <VAULT_PDA> --upgrade-authority ./authority.json --skip-new-upgrade-authority-signer-check`
3. Upgrade through multisig governance (this tool)
4. Extend capacity through multisig governance (this tool - uses `ExtendProgramChecked`)

**Note:** Authority transfer FROM multisig back to single-sig is not currently supported due to CPI coordination constraints. Once transferred to multisig, programs remain under governance control.

---

### Multisig Management (`multisig`)

Create and manage Squads V4 multisig vaults. Provides `spherenet-authority` crate integration.

**Example: Create Multisig**

```bash
spherenet-admin multisig create \
  --members <PUBKEY>,<PUBKEY>,<PUBKEY> \
  --threshold 2 \
  --create-key ./multisig-create-key.json \
  --payer ~/.config/solana/id.json \
  --memo "Treasury multisig 2-of-3"
```

**Proposal Workflow:**

```bash
# Approve (requires threshold members)
spherenet-admin multisig approve \
  --multisig <MULTISIG_PDA> \
  --transaction-index <INDEX> \
  --member ./member.json

# Execute (any member can execute once threshold met)
spherenet-admin multisig execute \
  --multisig <MULTISIG_PDA> \
  --transaction-index <INDEX> \
  --member ./member.json
```

**Other Commands:**
- `multisig program-config-init` - One-time Squads program setup
- `multisig show --multisig <PDA>` - View multisig info (members, threshold, balance)

---

### Transfer & Airdrop

**Transfer SOL:**
```bash
# Single-sig
spherenet-admin transfer --destination <PUBKEY> --amount 1.5 --from ./keypair.json

# Multi-sig (creates proposal)
spherenet-admin transfer --destination <PUBKEY> --amount 0.5 --multisig <MULTISIG_PDA> --multisig-authority ./member.json
```

**Airdrop (testnet only):**
```bash
spherenet-admin airdrop --pubkey <PUBKEY> --amount 5.0
```

## Architecture

The CLI uses:
- `spherenet-validator-whitelist-client` - Generated instruction builders for validator whitelist operations
- `spherenet-program-whitelist-client` - Generated instruction builders for program whitelist operations
- `spherenet-authority` - Authority abstraction and Squads v4 multisig integration

All instruction builders are generated from their respective interface definitions using Codama/Kinobi.

## Network Information

**Testnet:**
- RPC: `https://api.testnet.sphere.net`
- Validator Whitelist Program: `wLpnFMEvuP6hPE84AGrsmNr2Bo2uk69MC4kKWtrWHBN`
- Program Whitelist Program: `PwLzPtX2e5PwNFQTrEY5DkkmRQ1t1x5vWcGzTnMgibR`
- Squads v4 Program: `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`
