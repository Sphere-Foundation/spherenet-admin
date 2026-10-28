#!/usr/bin/env bash
#
# write-stake-buffer.sh <config.json> <localnet|testnet> [--lock] — stage a
# spherenet-stake release .so into the Loader-v3 buffer a feature-gated stake
# program upgrade consumes (`upgrade_core_bpf_program`), then lock it.
#
# One config per release/buffer, naming the gate that consumes it
# (scripts/buffers/<name>.json; git-ignored so it
# can hold a local keypair path or kms:// URI). See scripts/buffers/example.json.
#
#   1. download the release .so (+ .sha256) and verify it against the pinned sha256
#   2. check the buffer key's pubkey is the buffer address the client binary
#      expects (the gate's `buffer::id()`)
#   3. `program write-buffer`, leaving the payer as the buffer authority
#   4. `program show-buffer`: the on-chain bytes must hash to the pinned sha256
#   5. with --lock only: set the authority to 11111111111111111111111111111111
#      (asks for confirmation; irreversible)
#
# Lock last: until step 5 a wrong write can be fixed (re-run to overwrite, or
# `program close-buffer` and start over). After it, the buffer is immutable.
#
# Why 1111…1: genesis `--upgradeable-program … none` stores the stake program's
# upgrade authority as Some(Pubkey::default()) = Some(1111…1), and
# `upgrade_core_bpf_program` requires the buffer's authority to match exactly
# (else UpgradeAuthorityMismatch). Nobody can sign as it.
#
# Re-running is safe: the script reads the buffer first. Missing → write; holding
# the expected bytes → skip the write (go straight to verify / --lock); holding
# other bytes → rewrite; locked → only check the bytes. A buffer whose authority
# isn't the payer is refused.
#
# Per-target defaults (override in the config's "targets" block, or with the
# URL / PAYER env vars):
#   localnet  http://127.0.0.1:8899; payer = the client's opt/faucet keypair
#   testnet   https://api.test.sphere.net; payer = ~/.config/solana/id.json
#
# Other env: CLIENT_DIR (spherenet-client checkout, default ../spherenet-client),
# ADMIN_BIN (prebuilt admin binary; default: built once from this repo).
#
# Prereqs: gh (authenticated), jq, solana on PATH, a funded payer, gcloud ADC for
# kms:// keys (and, for localnet, a running `origami run localnet`).
#
set -euo pipefail

SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPTS_DIR/.." && pwd)"
LOCKED_AUTHORITY="11111111111111111111111111111111"

usage() { echo "usage: $(basename "$0") <config.json> <localnet|testnet> [--lock]" >&2; exit 1; }
die() { echo "❌ $*" >&2; exit 1; }

CONFIG="${1:-}"; TARGET="${2:-}"; LOCK="${3:-}"
[[ -n "$CONFIG" && -n "$TARGET" ]] || usage
[[ -z "$LOCK" || "$LOCK" == "--lock" ]] || usage
[[ -f "$CONFIG" ]] || die "config not found: $CONFIG"

cfg() { jq -er "$1" "$CONFIG" || die "config $CONFIG is missing $1"; }
cfg_opt() { jq -r "$1 // empty" "$CONFIG"; }

NAME="$(cfg .name)"
GATE_NAME="$(cfg .gate.name)"
GATE_KEY="$(cfg .gate.key)"
STAKE_REPO="$(cfg .release.repo)"
STAKE_TAG="$(cfg .release.tag)"
STAKE_ASSET="$(cfg .release.asset)"
EXPECTED_SHA="$(cfg .release.sha256)"
BUFFER_KEYPAIR="$(cfg .buffer.keypair)"
BUFFER_ADDRESS="$(cfg .buffer.address)"
# Relative keypair paths are relative to this repo.
[[ "$BUFFER_KEYPAIR" == kms://* || "$BUFFER_KEYPAIR" == /* ]] || BUFFER_KEYPAIR="$REPO_ROOT/$BUFFER_KEYPAIR"

case "$TARGET" in
  localnet)
    CLIENT_DIR="${CLIENT_DIR:-$REPO_ROOT/../spherenet-client}"
    URL="${URL:-$(cfg_opt .targets.localnet.url)}"; URL="${URL:-http://127.0.0.1:8899}"
    PAYER="${PAYER:-$(cfg_opt .targets.localnet.payer)}"; PAYER="${PAYER:-$CLIENT_DIR/opt/faucet/id.json}"
    ;;
  testnet)
    URL="${URL:-$(cfg_opt .targets.testnet.url)}"; URL="${URL:-https://api.test.sphere.net}"
    PAYER="${PAYER:-$(cfg_opt .targets.testnet.payer)}"; PAYER="${PAYER:-$HOME/.config/solana/id.json}"
    ;;
  *) usage ;;
esac

# Build admin once, up front and visibly (unless ADMIN_BIN is given), so no
# later step compiles silently.
if [[ -z "${ADMIN_BIN:-}" ]]; then
  echo "🔨 Building spherenet-admin"
  ( cd "$REPO_ROOT" && cargo build --quiet ) || die "admin build failed"
  ADMIN_BIN="$REPO_ROOT/target/debug/spherenet-admin"
fi
admin() { "$ADMIN_BIN" --url "$URL" "$@"; }

# Pubkey of a keypair file or kms:// URI (the URI carries it as ?pubkey=).
pubkey_of() {
  if [[ "$1" == kms://* ]]; then
    [[ "$1" == *"pubkey="* ]] || die "kms:// URI has no ?pubkey=: $1"
    echo "${1##*pubkey=}"
  else
    [[ -f "$1" ]] || die "keypair file not found: $1"
    solana address -k "$1"
  fi
}

# show-buffer as JSON, or empty if the buffer doesn't exist yet. A missing
# buffer is not an error: show-buffer prints {"found": false, ...} and exits 0.
# Anything else that fails (RPC, bad account) stops the script.
show_buffer() {
  local out
  out="$(admin --output json program show-buffer "$BUFFER_ADDRESS")" \
    || die "show-buffer failed for $BUFFER_ADDRESS (see the error above)"
  [[ "$(jq -r '.found' <<<"$out")" == "false" ]] && return 0
  echo "$out"
}

echo "🧾 $NAME → $TARGET ($URL)"
echo "🚪 Gate: $GATE_NAME ($GATE_KEY)"

# 1. Fetch + verify the release .so
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
echo "📥 Downloading ${STAKE_ASSET} from ${STAKE_REPO}@${STAKE_TAG}"
gh release download "$STAKE_TAG" --repo "$STAKE_REPO" \
  -p "$STAKE_ASSET" -p "${STAKE_ASSET}.sha256" -D "$WORK"
( cd "$WORK" && shasum -a 256 -c "${STAKE_ASSET}.sha256" >/dev/null ) \
  || die ".so does not match the release's .sha256"
SO_SHA="$(shasum -a 256 "$WORK/$STAKE_ASSET" | awk '{print $1}')"
[[ "$SO_SHA" == "$EXPECTED_SHA" ]] || die ".so sha256 $SO_SHA != pinned $EXPECTED_SHA"
echo "✅ .so verified ($SO_SHA)"

# 2. The buffer key must be the address the client binary expects
KEY_PK="$(pubkey_of "$BUFFER_KEYPAIR")"
[[ "$KEY_PK" == "$BUFFER_ADDRESS" ]] \
  || die "buffer key pubkey $KEY_PK != configured buffer address $BUFFER_ADDRESS"
PAYER_PK="$(pubkey_of "$PAYER")"
echo "🔑 Buffer $BUFFER_ADDRESS · payer $PAYER_PK"

# An already-locked buffer can only be checked, not rewritten.
# 3. Write the buffer, unless it already holds the expected bytes. The payer
#    stays its authority until the lock.
STATE="$(show_buffer)"
if [[ -z "$STATE" ]]; then
  echo "📦 No buffer yet — writing (authority stays $PAYER_PK until --lock)"
  WRITE=1
else
  ON_CHAIN_SHA="$(jq -r .sha256 <<<"$STATE")"
  ON_CHAIN_AUTH="$(jq -r .buffer_authority <<<"$STATE")"
  if [[ "$ON_CHAIN_AUTH" == "$LOCKED_AUTHORITY" ]]; then
    [[ "$ON_CHAIN_SHA" == "$EXPECTED_SHA" ]] \
      || die "buffer is LOCKED with different bytes (sha256 $ON_CHAIN_SHA); it cannot be fixed"
    echo "🔒 Already locked with the expected bytes — nothing to do."
    exit 0
  fi
  [[ "$ON_CHAIN_AUTH" == "$PAYER_PK" ]] \
    || die "buffer exists with authority $ON_CHAIN_AUTH, not the payer $PAYER_PK; this payer can't write or lock it"
  if [[ "$ON_CHAIN_SHA" == "$EXPECTED_SHA" ]]; then
    echo "📦 Buffer already holds the expected bytes — skipping the write"
    WRITE=0
  else
    echo "📦 Buffer holds other bytes (sha256 $ON_CHAIN_SHA) — rewriting"
    WRITE=1
  fi
fi

if [[ "$WRITE" == 1 ]]; then
  # write-buffer requires an explicit authority; the payer keeps it until --lock.
  admin program write-buffer \
    --program-so "$WORK/$STAKE_ASSET" \
    --buffer-keypair "$BUFFER_KEYPAIR" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
fi

# 4. Verify what's on chain
STATE="$(show_buffer)"
[[ -n "$STATE" ]] || die "show-buffer found no buffer at $BUFFER_ADDRESS after the write"
ON_CHAIN_SHA="$(jq -r .sha256 <<<"$STATE")"
ON_CHAIN_AUTH="$(jq -r .buffer_authority <<<"$STATE")"
[[ "$ON_CHAIN_SHA" == "$EXPECTED_SHA" ]] || die "on-chain sha256 $ON_CHAIN_SHA != pinned $EXPECTED_SHA"
[[ "$ON_CHAIN_AUTH" == "$PAYER_PK" ]] || die "buffer authority is $ON_CHAIN_AUTH, expected the payer $PAYER_PK"
echo "✅ On-chain bytes verified ($(jq -r .byte_len <<<"$STATE") bytes, sha256 $ON_CHAIN_SHA)"

# 5. Lock (only when asked)
if [[ "$LOCK" != "--lock" ]]; then
  echo
  echo "Not locked. Check it, then lock with:"
  echo "  $0 $CONFIG $TARGET --lock"
  exit 0
fi

echo
echo "⚠️  Locking sets the authority of $BUFFER_ADDRESS on $TARGET to $LOCKED_AUTHORITY."
echo "    The buffer can never be written, closed or reassigned again."
read -r -p "Type LOCK to continue: " answer
[[ "$answer" == "LOCK" ]] || die "not locked"

admin program set-buffer-authority --buffer "$BUFFER_ADDRESS" --lock --authority "$PAYER"

STATE="$(show_buffer)"
[[ "$(jq -r .buffer_authority <<<"$STATE")" == "$LOCKED_AUTHORITY" ]] || die "lock did not take effect"
[[ "$(jq -r .sha256 <<<"$STATE")" == "$EXPECTED_SHA" ]] || die "sha256 changed during lock"
echo
echo "🔒 Locked."
echo "Target:            $TARGET ($URL)"
echo "Gate:              $GATE_NAME ($GATE_KEY)"
echo "Buffer:            $BUFFER_ADDRESS"
echo "Buffer authority:  $LOCKED_AUTHORITY"
echo "Program sha256:    $EXPECTED_SHA"
