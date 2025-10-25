#!/usr/bin/env bash
set -e

# Navigate to script directory to ensure relative paths work
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADMIN_CLI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CLIENT_DIR="$(cd "$ADMIN_CLI_DIR/../spherenet-client" && pwd)"

# Configuration
RPC_URL="https://api.testnet.sphere.net"
AUTHORITY_KEYPAIR="$CLIENT_DIR/opt/testnet/validator_whitelist_authority/id.json"
VOTE_KEYPAIR="$CLIENT_DIR/opt/local/validator_0_vote/id.json"
IDENTITY_KEYPAIR="$CLIENT_DIR/opt/local/validator_0_identity/id.json"

# Get vote account pubkey
VOTE_PUBKEY=$(solana-keygen pubkey "$VOTE_KEYPAIR")

echo "================================================"
echo "Adding Validator to Testnet Whitelist"
echo "================================================"
echo "Vote Account: $VOTE_PUBKEY"
echo "RPC URL: $RPC_URL"
echo ""

# Check authority balance
echo "Checking authority balance..."
AUTHORITY_PUBKEY=$(solana-keygen pubkey "$AUTHORITY_KEYPAIR")
BALANCE=$(solana balance "$AUTHORITY_PUBKEY" --url "$RPC_URL" 2>/dev/null | awk '{print $1}')

echo "Authority ($AUTHORITY_PUBKEY) balance: $BALANCE SOL"

# Airdrop if balance is low (less than 1 SOL)
if (( $(echo "$BALANCE < 1" | bc -l) )); then
    echo "Balance low, requesting airdrop..."
    cd "$ADMIN_CLI_DIR"
    cargo run --quiet -- vw airdrop --keypair "$AUTHORITY_KEYPAIR"
    echo "Airdrop successful"
else
    echo "Balance sufficient, skipping airdrop"
fi

echo ""

# Check if vote account already exists
echo "Checking if vote account exists..."
if solana account "$VOTE_PUBKEY" --url "$RPC_URL" &>/dev/null; then
    echo "Vote account already exists, skipping creation"
else
    echo "Creating vote account..."
    solana create-vote-account \
        "$VOTE_KEYPAIR" \
        "$IDENTITY_KEYPAIR" \
        "$AUTHORITY_KEYPAIR" \
        --commission 0 \
        --keypair "$AUTHORITY_KEYPAIR" \
        --url "$RPC_URL"
    echo "Vote account created successfully"
fi

echo ""

# Check if already whitelisted
echo "Checking if vote account is already whitelisted..."
cd "$ADMIN_CLI_DIR"
if cargo run --quiet -- vw list 2>/dev/null | grep -q "$VOTE_PUBKEY"; then
    echo "Vote account is already whitelisted, skipping"
    echo ""
    echo "================================================"
    echo "Validator already in whitelist"
    echo "================================================"
    echo "Vote Account: $VOTE_PUBKEY"
    echo ""
    echo "View with: cargo run -- vw list"
else
    echo "Adding vote account to whitelist..."
    cargo run --quiet -- vw add "$VOTE_PUBKEY" --keypair "$AUTHORITY_KEYPAIR"

    echo ""
    echo "================================================"
    echo "Validator successfully added to whitelist!"
    echo "================================================"
    echo "Vote Account: $VOTE_PUBKEY"
    echo ""
    echo "Verify with: cargo run -- vw list"
fi
