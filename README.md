# SphereNet Admin CLI

Command-line tool for SphereNet governance and network administration.

```bash
cargo install spherenet-admin
spherenet-admin --help
```

Or download a prebuilt binary for Linux (x86_64) or macOS (Apple Silicon) from the [Releases page](https://github.com/Sphere-Foundation/spherenet-admin/releases/latest). Each archive ships the `spherenet-admin` binary plus a `.sha256` checksum — extract it and put it on your `PATH`:

```bash
tar -xzf spherenet-admin-<version>-<target>.tar.gz
sudo mv spherenet-admin-<version>-<target>/spherenet-admin /usr/local/bin/
```

Or build from source:

```bash
git clone https://github.com/Sphere-Foundation/spherenet-admin
cd spherenet-admin
cargo build --release        # binary at target/release/spherenet-admin
```

Global flags (all commands): `--url <URL>` (defaults to testnet `https://api.test.sphere.net`) and `--output text|json`. In both modes stdout carries only the result and diagnostics go to stderr, so `… --output json | jq` always sees clean JSON.

Every governance/program/transfer command runs **single-sig** (`--authority <keypair>`, executes immediately) or **multisig** (`--multisig <create-key> --multisig-authority <member>`, creates a Squads proposal). A multisig is always referenced by its **create-key** (a pubkey), which the CLI resolves + validates to the vault that holds funds and signs.

---

## Public commands

Everything a validator, deployer, or user runs on their own — no governance authority required.

### Join the network (`vw request` / `pw request`)

Onboarding is **two-step**: you *request*, an authority *approves* async. The requester co-signs to prove control of the key.

```bash
# Validator: request a validator-whitelist entry (the vote account co-signs)
spherenet-admin vw request ./vote-account.json --start-epoch 100 --end-epoch 200 --authority ./payer.json

# Deployer: request program-deploy access (the deploy authority co-signs)
spherenet-admin pw request ./deploy-authority.json --authority ./payer.json
```

The `--authority` here is just the fee-payer; the requesting key (vote account / deploy authority) is the co-signer that proves control. Once an admin approves, you're active.

### Vote (`vote`)

```bash
# minimal — every other role defaults to the identity keypair
spherenet-admin vote create --vote-account ./vote.json --identity ./identity.json
```
- `vote show <VOTE_ACCOUNT>` · `vote withdraw --vote-account <PK> --destination <PK> (--all) --withdraw-authority ./w.json`

Produces a **V4 vote account with a BLS voter key in one command** — the default path creates with the legacy `VoteInit` and appends the BLS key inline via `authorize_checked` (needs the `bls_pubkey_management_in_vote_account` feature active). Pass `--vote-init-v2` to set the BLS key at creation instead (`VoteInitV2`, requires SIMD-0464).

Optional overrides: `--authorized-voter <KEYPAIR>` (signs the BLS append and is what the BLS key is derived from — defaults to the identity), `--authorized-withdrawer <PK>` (defaults to the voter → identity; keep it a **distinct cold key**), `--from` / `--payer` (default to the identity), `--commission <0-100>` (default 100).

### Stake (`stake`)

```bash
spherenet-admin stake create --stake-account ./stake.json --amount 10000 \
  --stake-authority <PK> --withdraw-authority <PK> --from ./funder.json --payer ./payer.json
spherenet-admin stake delegate --stake-account <PK> --vote-account <PK> --stake-authority ./staker.json --payer ./payer.json
```
- `stake show <PK>` · `stake deactivate …` · `stake withdraw … (--amount <SPHR> | --all)`
- `stake deactivate --stake-account <PK> --force --payer ./payer.json` — permissionlessly deactivate stake still delegated to a validator that was removed from the whitelist (no stake authority needed)

Delegation is gated on the validator whitelist and fails fast if the vote account isn't approved.

### Programs (`program`)

The upgrade authority must be `pw`-approved first (see *Join the network*).

```bash
spherenet-admin program deploy --program-so ./p.so --program-keypair ./p-keypair.json \
  --upgrade-authority ./auth.json --payer ~/.config/solana/id.json [--max-data-len 500000]
```
- `program upgrade --program-id <ID> --program-so ./p.so …` (single-sig or multisig)
- `program extend --program-id <ID> --bytes <N>` — grow capacity (permissionless; payer covers the added rent)
- `program set-upgrade-authority --program-id <ID> [--new-authority <PK> | --new-multisig <CK> | --final]`

### Multisig (`multisig`)

```bash
spherenet-admin multisig create --members <PK>,<PK>,<PK> --threshold 2 --create-key ./ck.json --payer ./payer.json --memo "Treasury"
spherenet-admin multisig show    --multisig <CREATE_KEY>
spherenet-admin multisig approve --multisig <CREATE_KEY> --transaction-index <N> --member ./member.json
spherenet-admin multisig execute --multisig <CREATE_KEY> --transaction-index <N> --member ./member.json
```

### Utilities

```bash
spherenet-admin transfer --amount 1.5 --to <PK> --from ./keypair.json     # or --to-multisig <CREATE_KEY>
spherenet-admin airdrop  --pubkey <PK> --amount 5.0                        # testnet faucet
spherenet-admin balance  <PK>
spherenet-admin epoch
spherenet-admin server --port 8080                                        # read-only JSON HTTP API (GET /mp /vw /pw /epoch /balance/{pk} …)
```

---

## Admin commands

Governance-authority actions. Three whitelists/policies govern the network:

- **Validator Whitelist (`vw`)** — which validators may join consensus.
- **Program Whitelist (`pw`)** — which authorities may deploy/upgrade programs.
- **Monetary Policy (`mp`)** — inflation rate, per-signature fee, fee burn, and per-epoch VAT.

Each is a single config account with a transferable authority; `vw`/`pw` entries move through a **request → approve/reject** lifecycle.

### Validator Whitelist (`vw`)

```bash
spherenet-admin vw approve <VOTE_ACCOUNT> --start-epoch 100 --end-epoch 200 --authority ./authority.json
spherenet-admin vw reject  <VOTE_ACCOUNT> --authority ./authority.json
```
- `vw show` — authority + entries (each `Pending` or `Approved`)
- `vw remove <VOTE_ACCOUNT>` · `vw update-start-epoch <VA> --epoch <N>` · `vw update-end-epoch <VA> --epoch <N>`
- `vw propose-authority [--new-authority <PK> | --new-multisig <CK>]` · `vw accept-authority` · `vw cancel-authority`

### Program Whitelist (`pw`)

```bash
spherenet-admin pw approve <DEPLOYER_PUBKEY> --authority ./authority.json
spherenet-admin pw reject  <DEPLOYER_PUBKEY> --authority ./authority.json
```
- `pw show` · `pw remove <DEPLOYER>`
- `pw propose-authority [--new-authority | --new-multisig]` · `pw accept-authority` · `pw cancel-authority`

### Monetary Policy (`mp`)

```bash
spherenet-admin mp update-inflation-rate <BIPS> --authority ./authority.json    # 0–2000 = 0–20%
```
- `mp show`
- `mp update-lamports-per-signature <LAMPORTS>` · `mp update-burn-percent <PERCENT>` · `mp update-vat-lamports-per-epoch <LAMPORTS>`
- `mp propose-authority [--new-authority | --new-multisig]` · `mp accept-authority` · `mp cancel-authority`

The `mp` config account is created at genesis, not by this CLI.

### Signing with GCP Cloud KMS (`kms://`)

Anywhere a single-sig command takes a keypair path (`--authority`, `--from`,
program `--upgrade-authority`), a `kms://` URI can be passed instead to sign
with an Ed25519 key held in Google Cloud KMS — the private key never touches
disk:

```bash
spherenet-admin vw approve <VOTE_ACCOUNT> \
  --authority "kms://projects/<p>/locations/<l>/keyRings/<r>/cryptoKeys/<k>/cryptoKeyVersions/<v>?pubkey=<BASE58_ADDRESS>"
```

The URI names the exact crypto-key version (the key must use algorithm
`EC_SIGN_ED25519`) and the Solana address the key is expected to have; every
signature returned by KMS is verified against that address before use. To get
the address from a key's public-key PEM:

```bash
gcloud kms keys versions get-public-key <V> --key <K> --keyring <R> \
  --location <L> --project <P> --output-file pubkey.pem
spherenet-admin kms address pubkey.pem
```

Authentication uses Application Default Credentials: run
`gcloud auth application-default login`, or point
`GOOGLE_APPLICATION_CREDENTIALS` at a service-account key. The caller needs
`cloudkms.cryptoKeyVersions.useToSign` on the key (e.g. role
`roles/cloudkms.signerVerifier`). Before building a transaction, the CLI
preflights the key (access, algorithm, and that its address matches the URI's
`pubkey`) — but the sign permission itself can only be proven by signing, so
a caller with only `roles/cloudkms.publicKeyViewer` passes preflight and
fails at signing time.

KMS signing is supported for single-sig authorities; the payer, co-signer,
and account-keypair arguments throughout still require local keypair files.

---

## Built-in addresses

Testnet RPC: `https://api.test.sphere.net`

| program | address |
|---|---|
| Stake | `Stake11111111111111111111111111111111111111` |
| Vote | `Vote111111111111111111111111111111111111111` |
| Validator Whitelist | `VwL1111111111111111111111111111111111111111` |
| Program Whitelist | `PwL1111111111111111111111111111111111111111` |
| Monetary Policy | `MpM1111111111111111111111111111111111111111` |
| Squads v4 | `Sqds111111111111111111111111111111111111111` |
| ZK Loader | `ZkLoader11111111111111111111111111111111111` |
| SPL Token | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` |
| Token-2022 | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` |
| Associated Token Account | `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL` |
