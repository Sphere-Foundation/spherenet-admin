#!/usr/bin/env bash
#
# test-write-buffer.sh <1..8> — integration test for the `spherenet-admin program`
# buffer lifecycle: write-buffer, show-buffer, set-buffer-authority, close-buffer.
#
# Runs against a live cluster (testnet by default). Each phase stages a buffer (or
# deploys a program), verifies it, and closes it again with `close-buffer` to
# reclaim rent — so re-runs cost only tx fees (except phase 8). Phases:
#
#   1  write + verify ...... a rejected --buffer-authority fails before anything is
#                            created; then stage the .so, read the buffer back, check
#                            the on-chain bytes hash to the .so (payer IS the authority)
#   2  resume .............. run the same --buffer-keypair twice; the 2nd run should
#                            report "already staged, nothing to do"
#   3  transfer branch ..... payer != buffer-authority; closing the buffer with the
#                            authority key proves custody actually transferred
#   4  deploy + upgrade ..... deploy the fixture program, then upgrade it with the
#                            same .so (exercises the shared write_buffer via
#                            deploy/upgrade). Needs an APPROVED deployer.
#   5  show-buffer .......... show-buffer's JSON (sha256 / byte_len / authority)
#                            matches write-buffer's JSON and the local .so
#   6  set-authority ........ hand the buffer to a second key and back; while the
#                            second key holds it, write-buffer must refuse to resume
#   7  close + re-create .... close-buffer, show-buffer reports not found, then
#                            write-buffer re-creates it at the SAME address
#   8  lock ................. a raw --new-authority 1111…1 is refused; --lock locks it;
#                            close-buffer and write-buffer then refuse. ⚠️ The locked
#                            buffer's rent is unrecoverable (~a few tenths of a SPHR).
#
# Prereqs: a funded PAYER on the target cluster; solana, solana-keygen, jq on PATH.
#
# Authority-signed admin commands (set-buffer-authority, close-buffer) are paid by
# the signing authority itself, so throwaway authority keys get a small transfer
# from PAYER first (see `fund`).
#
# Config comes from tests/.env (sourced below); exported env vars override it:
#   URL / PAYER / UPGRADE_AUTHORITY — see tests/.env
#   PROGRAM_SO           the .so to stage (default: the committed tests/fixtures fixture)
#   ADMIN_BIN            admin binary to test (default: this repo's target/release/
#                        spherenet-admin, built once at startup so it is never stale)
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

USAGE="usage: $(basename "$0") <1..8>   (1=write+verify 2=resume 3=transfer 4=deploy+upgrade 5=show 6=set-authority 7=close+recreate 8=lock)"
case "${1:-}" in
  [1-8]) ;;
  *) echo "$USAGE" >&2; exit 2 ;;
esac

# Build the release binary once and call it directly (a `cargo run` per admin call
# is slow across ~10 calls per phase). Building first means the binary always
# matches the checked-out source — a leftover release build would test stale code.
if [[ -z "${ADMIN_BIN:-}" ]]; then
  echo "⏳ building spherenet-admin (release)…" >&2
  ( cd "$REPO_ROOT" && cargo build --release --quiet )
  ADMIN_BIN="$REPO_ROOT/target/release/spherenet-admin"
fi
[[ -x "$ADMIN_BIN" ]] || { echo "❌ admin binary not found: $ADMIN_BIN" >&2; exit 1; }

[[ -f "$PROGRAM_SO" ]] || { echo "❌ .so not found: $PROGRAM_SO" >&2; exit 1; }
PAYER_PK="$(solana address -k "$PAYER")"
LOCAL_SHA="$(shasum -a 256 "$PROGRAM_SO" | awk '{print $1}')"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# Run the admin CLI under test against $URL.
admin() { "$ADMIN_BIN" --url "$URL" "$@"; }

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

# Phase header, set off by a blank line before and after.
section() { printf '\n── %s ──\n\n' "$1"; }

# Close a buffer with admin's own close-buffer (exercising it in every phase);
# rent always goes back to PAYER. The authority signs and pays the fee.
close() { # <buffer_pubkey> <authority_keypair>
  admin program close-buffer --buffer "$1" --authority "$2" --recipient "$PAYER_PK" >/dev/null
}

newkey() { solana-keygen new --no-bip39-passphrase --force --silent -o "$1" >/dev/null; }

# Give a throwaway authority enough SPHR to pay its own tx fees.
fund() { # <pubkey>
  solana transfer "$1" 0.01 --url "$URL" -k "$PAYER" --allow-unfunded-recipient >/dev/null
}

# Run admin with --output json and print stdout only (progress goes to stderr).
admin_json() { admin --output json "$@" 2>/dev/null; }

# Assert a command FAILS and its output contains <needle>.
expect_fail() { # <needle> <cmd...>
  local needle="$1"; shift
  local out
  if out="$("$@" 2>&1)"; then
    echo "❌ expected failure (\"$needle\") but the command succeeded"; exit 1
  fi
  if [[ "$out" == *"$needle"* ]]; then echo "✅ refused as expected: \"$needle\""
  else echo "❌ failed, but without \"$needle\":"; echo "$out" | tail -5; exit 1; fi
}

# Assert two values are equal.
expect_eq() { # <label> <actual> <expected>
  if [[ "$2" == "$3" ]]; then echo "✅ $1: $2"
  else echo "❌ $1: got $2, expected $3"; exit 1; fi
}

echo "payer $PAYER_PK | .so sha ${LOCAL_SHA:0:16}… | $URL" >&2

phase1() {
  section "PHASE 1: write + verify (payer is buffer-authority)"
  newkey "$WORK/buf.json"; buf="$(solana address -k "$WORK/buf.json")"

  # The hand-off target is validated BEFORE anything is created: a rejected target
  # (here a malformed pubkey; a Squads create-key / config account fails at the same
  # point) must leave no buffer behind. A regression that resolved the target after
  # the create would strand a funded buffer under an unusable authority.
  echo "⏳ a rejected --buffer-authority fails before the buffer is created"
  expect_fail "Invalid pubkey" admin program write-buffer \
    --program-so "$PROGRAM_SO" --buffer-keypair "$WORK/buf.json" \
    --buffer-authority not-a-pubkey --payer "$PAYER"
  expect_eq "buffer created? (found)" "$(admin_json program show-buffer "$buf" | jq -r .found)" "false"

  echo "⏳ write"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
  verify_onchain "$buf"
  close "$buf" "$PAYER"; echo "✅ closed, rent reclaimed"
}

phase2() {
  section "PHASE 2: resume (run twice; 2nd should no-op)"
  newkey "$WORK/buf.json"; buf="$(solana address -k "$WORK/buf.json")"
  echo "⏳ first run"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
  verify_onchain "$buf"
  echo "⏳ second run (watch for 'already staged, nothing to do')"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER"
  close "$buf" "$PAYER"; echo "✅ closed"
}

phase3() {
  section "PHASE 3: transfer branch (payer != buffer-authority)"
  newkey "$WORK/buf.json";  buf="$(solana address -k "$WORK/buf.json")"
  newkey "$WORK/auth.json"; auth_pk="$(solana address -k "$WORK/auth.json")"
  echo "⏳ buffer-authority = throwaway $auth_pk"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$auth_pk" \
    --payer "$PAYER"
  verify_onchain "$buf"
  # close needs the authority's signature — success proves the transfer landed.
  fund "$auth_pk"
  close "$buf" "$WORK/auth.json"; echo "✅ closed by authority key — transfer confirmed"
}

phase4() {
  section "PHASE 4: deploy → upgrade cycle"
  newkey "$WORK/program.json"; prog="$(solana address -k "$WORK/program.json")"
  echo "⏳ program id $prog | upgrade-authority $UPGRADE_AUTHORITY"
  echo "⏳ (deploy/upgrade require the upgrade-authority to be an APPROVED deployer — see 'pw show')"

  echo "⏳ deploy"
  admin program deploy \
    --program-so "$PROGRAM_SO" \
    --program-keypair "$WORK/program.json" \
    --upgrade-authority "$UPGRADE_AUTHORITY" \
    --payer "$PAYER"
  solana program show "$prog" --url "$URL"

  echo "⏳ upgrade (same .so — 'Last Deployed In Slot' should advance)"
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

# Write a fresh buffer with PAYER as its authority; echoes the buffer pubkey.
# Leaves write-buffer's JSON result in $WORK/write.json.
write_fresh() { # <buffer_keypair_path>
  newkey "$1"
  # stderr stays visible so a failed write shows its error; `|| return` makes the
  # caller's `buf="$(write_fresh …)"` fail (and set -e stop the script).
  admin --output json program write-buffer \
    --program-so "$PROGRAM_SO" \
    --buffer-keypair "$1" \
    --buffer-authority "$PAYER_PK" \
    --payer "$PAYER" > "$WORK/write.json" || return 1
  solana address -k "$1"
}

phase5() {
  section "PHASE 5: show-buffer matches write-buffer"
  buf="$(write_fresh "$WORK/buf.json")"
  admin_json program show-buffer "$buf" > "$WORK/show.json"
  expect_eq "sha256 (show == write)" "$(jq -r .sha256 "$WORK/show.json")" "$(jq -r .sha256 "$WORK/write.json")"
  expect_eq "sha256 (show == .so)" "$(jq -r .sha256 "$WORK/show.json")" "$LOCAL_SHA"
  expect_eq "byte_len (show == write)" "$(jq -r .byte_len "$WORK/show.json")" "$(jq -r .byte_len "$WORK/write.json")"
  expect_eq "buffer_authority" "$(jq -r .buffer_authority "$WORK/show.json")" "$PAYER_PK"
  close "$buf" "$PAYER"; echo "✅ closed"
}

phase6() {
  section "PHASE 6: set-buffer-authority round trip + resume refusal"
  buf="$(write_fresh "$WORK/buf.json")"
  newkey "$WORK/auth2.json"; auth2_pk="$(solana address -k "$WORK/auth2.json")"
  fund "$auth2_pk"

  echo "⏳ payer → $auth2_pk"
  admin program set-buffer-authority --buffer "$buf" --new-authority "$auth2_pk" --authority "$PAYER"
  expect_eq "authority after hand-off" "$(admin_json program show-buffer "$buf" | jq -r .buffer_authority)" "$auth2_pk"

  echo "⏳ write-buffer must refuse to resume a buffer the payer no longer holds"
  expect_fail "can't be resumed" admin program write-buffer \
    --program-so "$PROGRAM_SO" --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" --payer "$PAYER"

  echo "⏳ the wrong key is refused before anything is sent"
  expect_fail "is held by" admin program set-buffer-authority \
    --buffer "$buf" --new-authority "$PAYER_PK" --authority "$PAYER"

  echo "⏳ $auth2_pk → payer"
  admin program set-buffer-authority --buffer "$buf" --new-authority "$PAYER_PK" --authority "$WORK/auth2.json"
  expect_eq "authority after hand-back" "$(admin_json program show-buffer "$buf" | jq -r .buffer_authority)" "$PAYER_PK"
  close "$buf" "$PAYER"; echo "✅ closed"
}

phase7() {
  section "PHASE 7: close-buffer + re-create at the same address"
  buf="$(write_fresh "$WORK/buf.json")"
  close "$buf" "$PAYER"; echo "⏳ closed"
  expect_eq "show-buffer after close (found)" "$(admin_json program show-buffer "$buf" | jq -r .found)" "false"

  echo "⏳ re-create at $buf with the same key"
  admin program write-buffer \
    --program-so "$PROGRAM_SO" --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" --payer "$PAYER"
  verify_onchain "$buf"
  close "$buf" "$PAYER"; echo "✅ closed"
}

phase8() {
  section "PHASE 8: lock"
  echo "⚠️  This leaves a LOCKED buffer whose rent can never be reclaimed."
  buf="$(write_fresh "$WORK/buf.json")"

  echo "⏳ a raw --new-authority 1111…1 is refused (locking must be explicit)"
  expect_fail "use --lock" admin program set-buffer-authority \
    --buffer "$buf" --new-authority 11111111111111111111111111111111 --authority "$PAYER"

  echo "⏳ --lock"
  admin program set-buffer-authority --buffer "$buf" --lock --authority "$PAYER"
  expect_eq "authority after lock" "$(admin_json program show-buffer "$buf" | jq -r .buffer_authority)" "11111111111111111111111111111111"

  echo "⏳ close-buffer and write-buffer must refuse a locked buffer"
  expect_fail "is locked" admin program close-buffer --buffer "$buf" --authority "$PAYER"
  expect_fail "It is locked" admin program write-buffer \
    --program-so "$PROGRAM_SO" --buffer-keypair "$WORK/buf.json" \
    --buffer-authority "$PAYER_PK" --payer "$PAYER"
  echo "✅ lock verified — buffer $buf stays locked (rent not reclaimable)"
}

case "${1:-}" in
  1) phase1 ;;
  2) phase2 ;;
  3) phase3 ;;
  4) phase4 ;;
  5) phase5 ;;
  6) phase6 ;;
  7) phase7 ;;
  8) phase8 ;;
  *) echo "$USAGE" >&2; exit 2 ;;
esac
