# SphereNet Admin CLI

Command-line tool for SphereNet governance and network administration.

## Overview

**Governance**
- **Validator Whitelist (`vw`)** — control which validators participate in consensus
- **Program Whitelist (`pw`)** — control which authorities can deploy/upgrade programs
- **Monetary Policy (`mp`)** — inflation rate, transaction fee, fee burn, VAT

**Validator lifecycle**
- **Vote (`vote`)** — create, show, withdraw
- **Stake (`stake`)** — create, delegate, deactivate, withdraw

**Programs (`program`)**
- Deploy, upgrade, extend, and set/renounce upgrade authority — with whitelist enforcement

**Multisig (`multisig`)**
- Create Squads v4 vaults and drive proposals (create/approve/execute)

**Utilities** — `transfer`, `airdrop`, `balance`, `epoch`, `server`

Every governance/program/transfer command runs in **single-sig** (executes immediately) or **multisig** (creates a Squads proposal) mode.

## Output modes

Every command takes a global `--output text|json` (default `text`):

```bash
spherenet-admin mp show                     # human-readable
spherenet-admin mp show --output json | jq  # machine-readable
```

Discipline: in both modes **stdout carries only the command result**; all progress and diagnostics go to **stderr**. So `… --output json | jq` always sees clean JSON. Reads render a typed view; writes render a `{ "status": "executed" | "proposal_created", … }` result.

## Configuration

Connects to SphereNet testnet by default; override with the global `--url`:

```bash
spherenet-admin --url http://localhost:8899 vw show   # local validator
spherenet-admin vw show                               # testnet (default)
```

## Multisig references (create-key)

A multisig is **always referenced by its create-key** (a pubkey) — never a raw PDA. The CLI resolves the create-key on-chain, validates it's a real multisig, and derives the two PDAs it needs:

- the **config account** (members/threshold), and
- the **vault** (index 0) — which holds funds and signs on the multisig's behalf.

This is what keeps funds and authority safe: money and authority always target the **vault**, never the config account. Three flags carry a create-key:

| flag | meaning |
|---|---|
| `--multisig <create-key>` | *act as* this multisig — sign/propose from its vault (with `--multisig-authority <member-keypair>`) |
| `--new-multisig <create-key>` | *assign* authority to this multisig (resolves to its vault) |
| `--to-multisig <create-key>` | *send funds* to this multisig (resolves to its vault) |

Raw-pubkey variants (`--to`, `--new-authority`) are guarded: passing a multisig config account or create-key by mistake is rejected with a pointer to the `*-multisig` flag.

## Commands

### Validator Whitelist (`vw`)

```bash
# Add a validator (single-sig)
spherenet-admin vw add <VOTE_ACCOUNT> --start-epoch 100 --end-epoch 200 --authority ./authority.json

# Add a validator (multisig — proposes from the vault)
spherenet-admin vw add <VOTE_ACCOUNT> --start-epoch 100 --end-epoch 200 \
  --multisig <CREATE_KEY> --multisig-authority ./member.json
```

- `vw show` — authority and entries
- `vw remove <VOTE_ACCOUNT>`
- `vw update-start-epoch <VOTE_ACCOUNT> --epoch <N>` / `vw update-end-epoch <VOTE_ACCOUNT> --epoch <N>`
- `vw propose-authority [--new-authority <PUBKEY> | --new-multisig <CREATE_KEY>]`
- `vw accept-authority` / `vw cancel-authority`

### Program Whitelist (`pw`)

Controls **who** can deploy/upgrade programs.

```bash
spherenet-admin pw add <DEPLOYER_PUBKEY> --authority ./authority.json
```

- `pw show`, `pw remove <DEPLOYER>`
- `pw propose-authority [--new-authority <PUBKEY> | --new-multisig <CREATE_KEY>]`, `pw accept-authority`, `pw cancel-authority`

### Monetary Policy (`mp`)

```bash
spherenet-admin mp update-inflation-rate-bips 500 --authority ./authority.json
```

- `mp show`
- `mp update-inflation-rate-bips <BIPS>` (0–2000 = 0–20%)
- `mp update-lamports-per-signature <LAMPORTS>`, `mp update-burn-percent <PERCENT>`, `mp update-vat-lamports-per-epoch <LAMPORTS>`
- `mp propose-authority [--new-authority | --new-multisig]`, `mp accept-authority`, `mp cancel-authority`

The `mp` config account is created at genesis, not by this CLI.

### Vote (`vote`)

Direct keypair-file signing (the new account + validator identity must sign — doesn't fit the multisig abstraction).

```bash
# Defaults to a V2 account (VoteInitV2), deriving a BLS key from the identity
spherenet-admin vote create \
  --vote-account ./vote-account.json --identity ./identity.json \
  --authorized-voter <PUBKEY> --authorized-withdrawer <PUBKEY> \
  --commission 100 --from ./funder.json --payer ./payer.json

# --no-bls: legacy V1 account, for networks where SIMD-0464 isn't active yet
spherenet-admin vote create ... --no-bls

# Withdraw (signed by the withdraw authority; --all drains & closes)
spherenet-admin vote withdraw --vote-account <PUBKEY> --destination <PUBKEY> --all \
  --withdraw-authority ./withdrawer.json --payer ./payer.json
```

`vote create` derives a BLS key deterministically from `--identity` and sets it via `VoteInitV2` by default. That requires the `vote-account-initialize-v2` feature (SIMD-0464) active on the target network — where it isn't, the V2 instruction is rejected, so pass `--no-bls` to create a legacy V1 account (no BLS key). `vote show` displays the compressed BLS pubkey when set.

Keep `--authorized-withdrawer` distinct from `--identity` (the identity is a hot key; a warning is printed if they match).

- `vote show <VOTE_ACCOUNT>`

### Stake (`stake`)

```bash
# Create + fund (authorities are pubkeys here)
spherenet-admin stake create --stake-account ./stake.json --amount 10000 \
  --stake-authority <STAKER_PUBKEY> --withdraw-authority <WITHDRAWER_PUBKEY> \
  --from ./funder.json --payer ./payer.json

# Delegate to a whitelisted vote account (staker signs; whitelist preflighted)
spherenet-admin stake delegate --stake-account <PK> --vote-account <PK> \
  --stake-authority ./staker.json --payer ./payer.json
```

- `stake show <STAKE_ACCOUNT>`
- `stake deactivate --stake-account <PK> --stake-authority ./staker.json --payer ./payer.json`
- `stake withdraw --stake-account <PK> --destination <PK> (--amount <SPHR> | --all) --withdraw-authority ./withdrawer.json --payer ./payer.json`

`--amount` at create is the total deposit (delegatable = amount − rent reserve). Delegation is gated on the validator whitelist and fails fast with a "run `vw add`" message if the vote account isn't whitelisted.

### Programs (`program`)

```bash
# Deploy (upgrade authority must be pw-whitelisted first)
spherenet-admin program deploy \
  --program-so ./target/deploy/my_program.so \
  --program-keypair ./target/deploy/my_program-keypair.json \
  --upgrade-authority ./authority.json --payer ~/.config/solana/id.json \
  --max-data-len 500000   # optional; leave headroom for future upgrades

# Upgrade via multisig (proposes; then approve + execute)
spherenet-admin program upgrade --program-id <ID> --program-so ./my_program.so \
  --multisig <CREATE_KEY> --multisig-authority ./member.json --payer ./member.json

# Hand upgrade authority to a multisig (→ vault, validated)
spherenet-admin program set-upgrade-authority --program-id <ID> \
  --new-multisig <CREATE_KEY> --authority ./authority.json

# Renounce upgradeability (program becomes immutable)
spherenet-admin program set-upgrade-authority --program-id <ID> --final --authority ./authority.json
```

- `program extend --program-id <ID> --bytes <N>` — grow capacity (multisig-safe; uses `ExtendProgramChecked`)

After transferring upgrade authority to a multisig, whitelist the vault (`pw add <VAULT>`) so it can upgrade.

### Multisig (`multisig`)

```bash
# Create a vault
spherenet-admin multisig create --members <PK>,<PK>,<PK> --threshold 2 \
  --create-key ./create-key.json --payer ~/.config/solana/id.json --memo "Treasury 2-of-3"

# Inspect / approve / execute (referenced by create-key)
spherenet-admin multisig show --multisig <CREATE_KEY>
spherenet-admin multisig approve --multisig <CREATE_KEY> --transaction-index <N> --member ./member.json
spherenet-admin multisig execute --multisig <CREATE_KEY> --transaction-index <N> --member ./member.json
```

`create` takes the create-key as a keypair (it signs creation); everywhere else `--multisig` is the create-key **pubkey**.

### Transfer & Airdrop

```bash
# Transfer to a raw address, or to a multisig's vault
spherenet-admin transfer --amount 1.5 --to <PUBKEY> --from ./keypair.json
spherenet-admin transfer --amount 0.5 --to-multisig <CREATE_KEY> --from ./keypair.json

# Transfer out of a multisig vault (proposes)
spherenet-admin transfer --amount 0.5 --to <PUBKEY> \
  --multisig <CREATE_KEY> --multisig-authority ./member.json

# Airdrop (testnet faucet)
spherenet-admin airdrop --pubkey <PUBKEY> --amount 5.0
```

### Balance & Epoch

```bash
spherenet-admin balance <PUBKEY>
spherenet-admin epoch
```

## Read API (`server`)

The read (`fetch`) layer doubles as a read-only HTTP JSON API — the seed of a governance dashboard (signing stays client-side):

```bash
spherenet-admin server --port 8080
```

Endpoints return the same JSON the CLI prints (append `?pretty` for pretty output):

```
GET  /mp · /vw · /pw · /epoch
GET  /balance/{pubkey} · /vote/{pubkey} · /stake/{pubkey}
POST /airdrop/{pubkey}   (testnet, keyless; ?amount=)
```

## Architecture

- Reads follow `fetch() → View → Render`; writes follow build-instruction → sign/execute → `emit`. Each domain module is `show.rs` (reads) / `run.rs` (writes) / `auth.rs` (authority ops) / `mod.rs` (wiring).
- `src/cli/authority.rs` — the `Authority` adapter: single-sig executes directly, multisig routes the instruction into a Squads proposal.
- `src/squads/` — a hand-rolled Squads v4 client (types, instruction builders, commands). `resolve()` validates a create-key and returns the multisig + vault; `classify()` detects whether a pubkey is a config account / create-key.
- Whitelist / monetary-policy instructions come from generated `*-client` crates; stake/vote from `spherenet-stake-interface` / `solana-vote-interface`.

## Network Information

**Testnet** — RPC `https://api.test.sphere.net`

| program | address |
|---|---|
| Validator Whitelist | `VwL1111111111111111111111111111111111111111` |
| Program Whitelist | `PwL1111111111111111111111111111111111111111` |
| Monetary Policy | `MpM1111111111111111111111111111111111111111` |
| Vote | `Vote111111111111111111111111111111111111111` |
| Stake | `Stake11111111111111111111111111111111111111` |
| Squads v4 | `Sqds111111111111111111111111111111111111111` |
