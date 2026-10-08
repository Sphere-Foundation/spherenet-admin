#!/usr/bin/env bash
#
# test-write-buffer.sh <1|2|3|4> — integration test for `spherenet-admin program write-buffer`.
#
# Runs against a live cluster (testnet by default). Each phase stages a buffer (or
# deploys a program), verifies it, and closes it again to reclaim rent — so re-runs
# cost only tx fees. Phases:
#
#   1  write + verify ...... stage the .so, read the buffer back, check the on-chain
#                            bytes hash to the .so   (payer IS the buffer-authority)
#   2  resume .............. run the same --buffer-keypair twice; the 2nd run should
#                            report "already staged, nothing to do"
#   3  transfer branch ..... payer != buffer-authority; closing the buffer with the
#                            authority key proves custody actually transferred
#   4  deploy + upgrade ..... deploy the fixture program, then upgrade it with the
#                            same .so (exercises the shared write_buffer via
#                            deploy/upgrade). Needs an APPROVED deployer.
#
# Prereqs: a funded PAYER on the target cluster; solana + solana-keygen on PATH.
#
# Config comes from tests/.env (sourced below); exported env vars override it:
#   URL / PAYER / UPGRADE_AUTHORITY — see tests/.env
#   PROGRAM_SO  the .so to stage (default: the committed tests/fixtures fixture)
#   ADMIN_BIN   prebuilt admin binary (default: `cargo run` from this repo)
#
set -euo pipefail

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/.." && pwd)"

# Shared test config (defaults; an exported env var still wins, see tests/.env).
[[ -f "$TESTS_DIR/.env" ]] && source "$TESTS_DIR/.env"

URL="${URL:-https://api.test.sphere.net}"
PAYER="${PAYER:-$HOME/.config/solana/id.json}"
PROGRAM_SO="${PROGRAM_SO:-$TESTS_DIR/fixtures/test_program.so}"
UPGRADE_AUTHORITY="${UPGRADE_AUTHORITY:-$PAYER}"

[[ -f "$PROGRAM_SO" ]] || { echo "❌ .so not found: $PROGRAM_SO" >&2; exit 1; }
PAYER_PK="$(solana address -k "$PAYER")"
LOCAL_SHA="$(shasum -a 256 "$PROGRAM_SO" | awk '{print $1}')"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# Run the admin CLI (prebuilt binary if given, else cargo run from the repo).
admin() {
  if [[ -n "${ADMIN_BIN:-}" ]]; then "$ADMIN_BIN" --url "$URL" "$@"
  else ( cd "$REPO_ROOT" && cargo run --quiet -- --url "$URL" "$@" ); fi
}

# Read a buffer back from chain and assert its ELF payload matches the .so.
# NOTE: `program write-buffer` already verifies this itself (fails closed) — this
# is a deliberate INDEPENDENT cross-check via a separate toolchain (solana account
# + shasum), so a bug in the command's own verify can't pass silently.
# Loader-v3 buffer data = 37-byte metadata (enum tag 4 + Option<Pubkey> 33) + ELF,
# so the payload starts at byte 38 (1-based).
verify_onchain() { # <buffer_pubkey>
  solana account "$1" --url "$URL" --output-file "$WORK/acct.bin" >/dev/null
  local sha; sha="$(tail -c +38 "$WORK/acct.bin" | shasum -a 256 | awk '{print $1}')"
  if [[ "$sha" == "$LOCAL_SHA" ]]; then echo "✅ on-chain bytes match .so ($sha)"
  else echo "❌ MISMATCH — on-chain $sha != .so $LOCAL_SHA"; exit 1; fi
}

close() { # <buffer_pubkey> <authority_keypair>
  solana program close "$1" --url "$URL" -k "$PAYER" --authority "$2" >/dev/null
}

newkey() { solana-keygen new --no-bip39-passphrase --force --silent -o "$1" >/dev/null; }

echo "payer $PAYER_PK | .so sha ${LOCAL_SHA:0:16}… | $URL" >&2

phase1() {
  echo "── PHASE 1: write + verify (payer is buffer-authority) ──"
  newkey "$WORK/buf.json"; buf="$(solana address -k "$WORK/buf.json")"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
  verify_onchain "$buf"
  close "$buf" "$PAYER"; echo "✅ closed, rent reclaimed"
}

phase2() {
  echo "── PHASE 2: resume (run twice; 2nd should no-op) ──"
  newkey "$WORK/buf.json"; buf="$(solana address -k "$WORK/buf.json")"
  echo "· first run"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
  verify_onchain "$buf"
  echo "· second run (watch for 'already staged, nothing to do')"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
  close "$buf" "$PAYER"; echo "✅ closed"
}

phase3() {
  echo "── PHASE 3: transfer branch (payer != buffer-authority) ──"
  newkey "$WORK/buf.json";  buf="$(solana address -k "$WORK/buf.json")"
  newkey "$WORK/auth.json"; auth_pk="$(solana address -k "$WORK/auth.json")"
  echo "· buffer-authority = throwaway $auth_pk"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$auth_pk" \
    --payer "$PAYER"
  verify_onchain "$buf"
  # close needs the authority's signature — success proves the transfer landed.
  close "$buf" "$WORK/auth.json"; echo "✅ closed by authority key — transfer confirmed"
}

phase4() {
  echo "── PHASE 4: deploy → upgrade cycle ──"
  newkey "$WORK/program.json"; prog="$(solana address -k "$WORK/program.json")"
  echo "· program id $prog | upgrade-authority $UPGRADE_AUTHORITY"
  echo "· (deploy/upgrade require the upgrade-authority to be an APPROVED deployer — see 'pw show')"

  echo "· deploy"
  admin program deploy \
    --program-so "$PROGRAM_SO" \
    --program-keypair "$WORK/program.json" \
    --upgrade-authority "$UPGRADE_AUTHORITY" \
    --payer "$PAYER"
  solana program show "$prog" --url "$URL"

  echo "· upgrade (same .so — 'Last Deployed In Slot' should advance)"
  admin program upgrade \
    --program-id "$prog" \
    --program-so "$PROGRAM_SO" \
    --upgrade-authority "$UPGRADE_AUTHORITY" \
    --payer "$PAYER"
  solana program show "$prog" --url "$URL"
  echo "✅ deploy + upgrade cycle complete for $prog"

  # Reclaim rent only if the authority is a local keypair we can sign close with.
  if [[ -f "$UPGRADE_AUTHORITY" ]]; then
    solana program close "$prog" --url "$URL" -k "$PAYER" --authority "$UPGRADE_AUTHORITY" --bypass-warning >/dev/null \
      && echo "✅ program closed, rent reclaimed"
  else
    echo "ℹ️  leaving $prog deployed (authority is kms://, cannot close via solana CLI) — close manually if needed"
  fi
}

case "${1:-}" in
  1) phase1 ;;
  2) phase2 ;;
  3) phase3 ;;
  4) phase4 ;;
  *) echo "usage: $(basename "$0") <1|2|3|4>   (1=write+verify  2=resume  3=transfer-branch  4=deploy+upgrade)" >&2; exit 2 ;;
esac
