# SphereNet Admin CLI

Command-line interface for administering SphereNet validator and program whitelists.

## Overview

SphereNet administration tool for governance and network management:

**Governance:**
- **Validator Whitelist** - Control which validators participate in consensus
- **Program Whitelist** - Control which authorities can deploy/upgrade programs
- **Monetary Policy** - Manage inflation rate, transaction fees, and fee burn percentage

**Validator Lifecycle:**
- **Vote Accounts** - Create and inspect vote accounts
- **Stake Accounts** - Create, delegate, deactivate, and withdraw stake

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
spherenet-admin --url http://localhost:8899 vw show  # Local validator
spherenet-admin vw show                               # Testnet (default)
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
- `vw show` - Show validator whitelist account (authority and entries)
- `vw remove <VOTE_ACCOUNT>` - Remove validator from whitelist
- `vw update-start-epoch <VOTE_ACCOUNT> --epoch <N>` - Update when validator can start
- `vw update-end-epoch <VOTE_ACCOUNT> --epoch <N>` - Update when validator term ends
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
- `pw show` - Show program whitelist account (authority and entries)
- `pw remove <DEPLOYER>` - Remove deployer authority
- `pw propose-authority <NEW_AUTHORITY>` - Initiate authority transfer
- `pw accept-authority` - Accept pending authority transfer
- `pw cancel-authority` - Cancel pending authority transfer

---

### Monetary Policy (`mp`)

Manage SphereNet's monetary policy parameters: inflation rate, transaction fees, and fee burn percentage.

**Example: Update Inflation Rate**

```bash
# Single-sig
spherenet-admin mp update-inflation-rate-bips 500 --authority ./authority.json

# Multi-sig
spherenet-admin mp update-inflation-rate-bips 500 \
  --multisig <MULTISIG_PDA> \
  --multisig-authority ./member.json
```

**Commands:**
- `mp show` - Show monetary policy account (authority and parameters)
- `mp create --authority <PATH> --payer <PATH>` - Create monetary policy account (testnet only)
- `mp update-inflation-rate-bips <BIPS>` - Update inflation rate (0-2000 bips = 0-20%)
- `mp update-lamports-per-signature <LAMPORTS>` - Update transaction fee
- `mp update-burn-percent <PERCENT>` - Update fee burn percentage (0-100%)
- `mp propose-authority <NEW_AUTHORITY>` - Initiate authority transfer
- `mp accept-authority` - Accept pending authority transfer
- `mp cancel-authority` - Cancel pending authority transfer

---

### Vote Accounts (`vote`)

Create and inspect validator vote accounts. Keypair-file based (not the multisig
authority abstraction) — the new account and the validator identity must sign.

**Example: Create Vote Account**

```bash
spherenet-admin vote create \
  --vote-account ./vote-account.json \
  --identity ./identity.json \
  --authorized-voter <PUBKEY> \
  --authorized-withdrawer <PUBKEY> \
  --commission 100 \
  --from ./funder.json \
  --payer ./payer.json
```

Roles are independent: `--authorized-withdrawer` should differ from `--identity`
(the identity is a hot key); a warning is printed if they match. Funds the
account with exactly the rent-exempt reserve. Uses the v1 `VoteInit` path
(no BLS; SIMD-0464/0387 inactive on testnet).

**Other Commands:**
- `vote show <VOTE_ACCOUNT>` - Show identity, authorities, commission, and voting state

---

### Stake Accounts (`stake`)

Create, delegate, and manage stake accounts — the validator activation path.
Keypair-file based; the relevant authority signs each operation.

**Example: Create + Delegate**

```bash
# 1. Create and fund a stake account (no vote account involved yet)
spherenet-admin stake create \
  --stake-account ./stake-account.json \
  --amount 10000 \
  --stake-authority <STAKER_PUBKEY> \
  --withdraw-authority <WITHDRAWER_PUBKEY> \
  --from ./funder.json \
  --payer ./payer.json

# 2. Delegate to a whitelisted vote account (staker signs; whitelist preflighted)
spherenet-admin stake delegate \
  --stake-account <STAKE_PUBKEY> \
  --vote-account <VOTE_PUBKEY> \
  --stake-authority ./staker.json \
  --payer ./payer.json
```

`--amount` is the total deposited (delegatable = amount − rent reserve). The
stake authority (staker) is a *pubkey* at create and a *keypair* (signer) at
delegate. Delegation is gated on the validator whitelist and preflighted with a
clear "run `vw add`" error if the vote account isn't whitelisted.

**Teardown:**

```bash
# Stop delegating (begins cooldown); staker signs
spherenet-admin stake deactivate --stake-account <PK> --stake-authority ./staker.json --payer ./payer.json

# Withdraw inactive lamports; withdraw authority signs (--all drains & closes)
spherenet-admin stake withdraw --stake-account <PK> --destination <PK> --all \
  --withdraw-authority ./withdrawer.json --payer ./payer.json
```

**Other Commands:**
- `stake show <STAKE_ACCOUNT>` - Show authorities, lockup, and delegation state

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
- `spherenet-monetary-policy-client` - Generated instruction builders for monetary policy operations
- `spherenet-stake-interface` - Stake program instructions (delegation carries the validator-whitelist entry PDA)
- `solana-vote-interface` - Vote program instructions and state (v1 `VoteInit` path)
- `spherenet-authority` - Authority abstraction and Squads v4 multisig integration

All whitelist/monetary-policy instruction builders are generated from their respective interface definitions using Codama/Kinobi. Vote and stake commands build instructions directly with keypair-file signing (the new account and authority keys must sign, which doesn't fit the multisig authority abstraction).

## Network Information

**Testnet:**
- RPC: `https://api.test.sphere.net`
- Validator Whitelist Program: `VwL1111111111111111111111111111111111111111`
- Program Whitelist Program: `PwL1111111111111111111111111111111111111111`
- Monetary Policy Program: `MpM1111111111111111111111111111111111111111`
- Vote Program: `Vote111111111111111111111111111111111111111`
- Stake Program: `Stake11111111111111111111111111111111111111`
- Squads v4 Program: `SqdsYSe3QC9aGdU5p8y7Y3HtT5VrVLEtYCNjN37yBTh`
