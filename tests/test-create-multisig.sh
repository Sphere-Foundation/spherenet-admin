#!/usr/bin/env bash
#
# test-create-multisig.sh <localnet|testnet> — utility: create a throwaway Squads
# multisig on the given cluster, for manual multisig checks (e.g. handing a buffer to
# its vault with `write-buffer --buffer-multisig`). The buffer test suite itself
# deliberately doesn't depend on Squads.
#
# Keyset: opt/test_multisig/{create_key,member_0,member_1,member_2}/{id.json,pubkey.txt}
#   - used as-is if present; any missing key is generated (opt/ is git-ignored)
#   - the SAME keyset on both clusters, so the create-key (and its multisig/vault
#     PDAs) are identical on localnet and testnet
#
# Idempotent: if the multisig already exists on the cluster it is just shown.
# On localnet, re-run after every fresh start (a restart wipes the chain).
#
# Targets (required — the target fixes the RPC URL, so a stale `export URL=…`
# in your shell can't send this to the wrong cluster):
#   localnet  http://127.0.0.1:8899; payer = the client's opt/faucet/id.json
#   testnet   https://api.test.sphere.net; payer = ~/.config/solana/id.json
#
# Exported env vars override the defaults:
#   PAYER          fee payer keypair         (default: per target, above)
#   CLIENT_DIR     spherenet-client checkout (default: sibling ../spherenet-client)
#   MULTISIG_KEYS  keyset directory          (default: opt/test_multisig)
#   THRESHOLD      approvals required        (default: 2, of 3 members)
#   ADMIN_BIN      admin binary (default: target/release/spherenet-admin, built once)
#
set -euo pipefail

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/.." && pwd)"

TARGET="${1:-}"
case "$TARGET" in
  localnet)
    CLIENT_DIR="${CLIENT_DIR:-$REPO_ROOT/../spherenet-client}"
    URL="http://127.0.0.1:8899"
    PAYER="${PAYER:-$CLIENT_DIR/opt/faucet/id.json}"
    ;;
  testnet)
    URL="https://api.test.sphere.net"
    PAYER="${PAYER:-$HOME/.config/solana/id.json}"
    ;;
  *)
    echo "usage: $(basename "$0") <localnet|testnet>" >&2; exit 2 ;;
esac
MULTISIG_KEYS="${MULTISIG_KEYS:-$REPO_ROOT/opt/test_multisig}"
THRESHOLD="${THRESHOLD:-2}"
MEMBERS=(member_0 member_1 member_2)

section() { printf '\n── %s ──\n\n' "$1"; }

[[ -f "$PAYER" ]] || { echo "❌ payer keypair not found: $PAYER" >&2; exit 1; }
echo "target $TARGET | $URL | payer $(solana address -k "$PAYER")" >&2

# Build the release binary once (same as test-write-buffer.sh) unless overridden.
if [[ -z "${ADMIN_BIN:-}" ]]; then
  echo "⏳ building spherenet-admin (release)…" >&2
  ( cd "$REPO_ROOT" && cargo build --release --quiet )
  ADMIN_BIN="$REPO_ROOT/target/release/spherenet-admin"
fi
[[ -x "$ADMIN_BIN" ]] || { echo "❌ admin binary not found: $ADMIN_BIN" >&2; exit 1; }
admin() { "$ADMIN_BIN" --url "$URL" "$@"; }

# Use <dir>/id.json if present, otherwise generate it; keep pubkey.txt in sync.
ensure_key() { # <name>
  local dir="$MULTISIG_KEYS/$1"
  mkdir -p "$dir"
  if [[ -f "$dir/id.json" ]]; then
    echo "⏳ using   $1"
  else
    solana-keygen new --no-bip39-passphrase --silent -o "$dir/id.json" >/dev/null
    chmod 600 "$dir/id.json"
    echo "⏳ created $1"
  fi
  solana address -k "$dir/id.json" > "$dir/pubkey.txt"
}

section "Keyset: $MULTISIG_KEYS"
ensure_key create_key
for m in "${MEMBERS[@]}"; do ensure_key "$m"; done

CREATE_KEY="$(cat "$MULTISIG_KEYS/create_key/pubkey.txt")"
MEMBER_PKS="$(for m in "${MEMBERS[@]}"; do cat "$MULTISIG_KEYS/$m/pubkey.txt"; done | paste -sd, -)"
echo "create-key: $CREATE_KEY"
echo "members:    $MEMBER_PKS  (threshold $THRESHOLD)"

section "Multisig on $URL"
if admin multisig show --multisig "$CREATE_KEY" >/dev/null 2>&1; then
  echo "⏳ already exists"
else
  echo "⏳ creating ${THRESHOLD}-of-${#MEMBERS[@]}"
  admin multisig create \
    --members "$MEMBER_PKS" \
    --threshold "$THRESHOLD" \
    --create-key "$MULTISIG_KEYS/create_key/id.json" \
    --payer "$PAYER"
fi
admin multisig show --multisig "$CREATE_KEY"

section "Use it"
echo "export MULTISIG_CREATE_KEY=$CREATE_KEY"
echo "member keys: $MULTISIG_KEYS/member_{0,1,2}/id.json"
