#!/usr/bin/env bash
#
# write-stake-buffer.sh <localnet|testnet> — stage the spherenet-stake release
# .so into a Loader-v3 buffer, for the feature-gated stake program upgrade
# (`spherenet::fix_stake_warmup_cooldown_rate`).
#
# Targets (both use the SAME buffer keypair, so the buffer address baked into
# the client is valid on either cluster):
#   localnet  http://127.0.0.1:8899; payer = the client's opt/faucet keypair
#   testnet   https://api.test.sphere.net; payer = ~/.config/solana/id.json
#
#   1. download the stake program release (.so + .sha256) and verify it
#   2. create the buffer keypair opt/stake-buffer.json (kept; never overwritten;
#      opt/ is git-ignored)
#   3. `spherenet-admin program write-buffer`, handing the buffer authority to
#      11111111111111111111111111111111 at the end
#
# Why that authority: genesis `--upgradeable-program … none` stores the stake
# program's upgrade authority as Some(Pubkey::default()) = Some(1111…1), not
# None. `upgrade_core_bpf_program` then requires the buffer's authority to match
# exactly, or fails with UpgradeAuthorityMismatch. Nobody can sign as 1111…1, so
# the buffer is also effectively immutable once written.
#
# The buffer pubkey printed at the end goes into the client's spherenet feature
# keys patch as `fix_stake_warmup_cooldown_rate::buffer`.
#
# Re-running is safe: an existing stake-buffer.json is reused, and write-buffer
# reports "already staged" if the buffer already holds the same bytes.
#
# Exported env vars override the per-target defaults:
#   URL               cluster RPC
#   PAYER             fee payer: keypair path or kms:// URI; signs every chunk
#   STAKE_TAG         spherenet-stake release tag
#   EXPECTED_SHA      pinned sha256 of the release .so (checked on top of the
#                     release's own .sha256)
#   BUFFER_KEYPAIR    buffer keypair path (default: opt/stake-buffer.json)
#   BUFFER_AUTHORITY  final buffer authority pubkey (must equal the stake
#                     program's upgrade authority)
#   CLIENT_DIR        spherenet-client checkout, for the localnet payer
#                     (default: sibling ../spherenet-client)
#   ADMIN_BIN         prebuilt admin binary (default: `cargo run` from this repo)
#
# Prereqs: gh (authenticated), solana + solana-keygen on PATH, a funded PAYER
# (and, for localnet, a running `run localnet` from the client).
#
set -euo pipefail

SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPTS_DIR/.." && pwd)"

TARGET="${1:-}"
case "$TARGET" in
  localnet)
    CLIENT_DIR="${CLIENT_DIR:-$REPO_ROOT/../spherenet-client}"
    URL="${URL:-http://127.0.0.1:8899}"
    PAYER="${PAYER:-$CLIENT_DIR/opt/faucet/id.json}"
    ;;
  testnet)
    URL="${URL:-https://api.test.sphere.net}"
    PAYER="${PAYER:-$HOME/.config/solana/id.json}"
    ;;
  *)
    echo "usage: $(basename "$0") <localnet|testnet>" >&2; exit 1 ;;
esac

STAKE_REPO="Sphere-Foundation/spherenet-stake"
STAKE_TAG="${STAKE_TAG:-v0.5.3}"
STAKE_ASSET="spherenet_stake_program.so"
EXPECTED_SHA="${EXPECTED_SHA:-a70d99b17f2226d411d94d7d86653ae02460cafaa733e9d6b8ed3d100c2e5eb5}"
BUFFER_KEYPAIR="${BUFFER_KEYPAIR:-$REPO_ROOT/opt/stake-buffer.json}"
BUFFER_AUTHORITY="${BUFFER_AUTHORITY:-11111111111111111111111111111111}"

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

admin() {
  if [[ -n "${ADMIN_BIN:-}" ]]; then "$ADMIN_BIN" --url "$URL" "$@"
  else ( cd "$REPO_ROOT" && cargo run --quiet -- --url "$URL" "$@" ); fi
}

# 1. Fetch + verify the release .so
echo "📥 Downloading ${STAKE_ASSET} from ${STAKE_REPO}@${STAKE_TAG}"
gh release download "$STAKE_TAG" --repo "$STAKE_REPO" \
  -p "$STAKE_ASSET" -p "${STAKE_ASSET}.sha256" -D "$WORK"

( cd "$WORK" && shasum -a 256 -c "${STAKE_ASSET}.sha256" ) \
  || { echo "❌ .so does not match the release's .sha256" >&2; exit 1; }
SO_SHA="$(shasum -a 256 "$WORK/$STAKE_ASSET" | awk '{print $1}')"
[[ "$SO_SHA" == "$EXPECTED_SHA" ]] \
  || { echo "❌ .so sha256 $SO_SHA != pinned $EXPECTED_SHA" >&2; exit 1; }
echo "✅ .so verified ($SO_SHA)"

# 2. Buffer keypair: create once, then always reuse
if [[ -f "$BUFFER_KEYPAIR" ]]; then
  echo "🔑 Reusing buffer keypair $BUFFER_KEYPAIR"
else
  echo "🔑 Creating buffer keypair $BUFFER_KEYPAIR"
  solana-keygen new --no-bip39-passphrase --silent -o "$BUFFER_KEYPAIR"
  chmod 600 "$BUFFER_KEYPAIR"
fi
BUFFER_PK="$(solana address -k "$BUFFER_KEYPAIR")"

# 3. Write the buffer
echo "📦 Writing buffer $BUFFER_PK on $TARGET ($URL), authority → $BUFFER_AUTHORITY"
admin program write-buffer \
  --program-so "$WORK/$STAKE_ASSET" \
  --buffer-keypair "$BUFFER_KEYPAIR" \
  --buffer-authority "$BUFFER_AUTHORITY" \
  --payer "$PAYER"

echo
echo "Target:            $TARGET ($URL)"
echo "Buffer:            $BUFFER_PK"
echo "Buffer authority:  $BUFFER_AUTHORITY"
echo "Program sha256:    $SO_SHA"
echo "Keep $BUFFER_KEYPAIR. Put the buffer pubkey in the client's"
echo "spherenet::fix_stake_warmup_cooldown_rate::buffer declare_id!."
