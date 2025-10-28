#!/usr/bin/env bash
set -euo pipefail

# SphereNet Testnet Health Check Script
# Performs comprehensive network health checks using RPC endpoints

RPC_URL="${RPC_URL:-https://api.testnet.sphere.net}"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper function to make RPC calls
rpc_call() {
    local method="$1"
    local params="${2:-[]}"
    curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":$params}" \
        "$RPC_URL"
}

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SphereNet Testnet Health Check${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 1. RPC Health Check
echo -e "${YELLOW}[1/8] Checking RPC Health...${NC}"
HEALTH=$(rpc_call "getHealth")
if echo "$HEALTH" | jq -e '.result == "ok"' > /dev/null 2>&1; then
    echo -e "  ${GREEN}✓${NC} RPC Status: OK"
else
    echo -e "  ${RED}✗${NC} RPC Status: UNHEALTHY"
    echo "$HEALTH" | jq .
    exit 1
fi

# 2. Version Info
VERSION=$(rpc_call "getVersion")
SOLANA_VERSION=$(echo "$VERSION" | jq -r '.result["solana-core"]')
FEATURE_SET=$(echo "$VERSION" | jq -r '.result["feature-set"]')
echo -e "  ${GREEN}✓${NC} Version: $SOLANA_VERSION (Feature Set: $FEATURE_SET)"
echo ""

# 3. Current Slot
echo -e "${YELLOW}[2/8] Checking Slot Progress...${NC}"
SLOT=$(rpc_call "getSlot" '[{"commitment":"finalized"}]')
CURRENT_SLOT=$(echo "$SLOT" | jq -r '.result')
echo -e "  ${GREEN}✓${NC} Current Finalized Slot: $CURRENT_SLOT"
echo ""

# 4. Epoch Info
echo -e "${YELLOW}[3/8] Checking Epoch Info...${NC}"
EPOCH_INFO=$(rpc_call "getEpochInfo")
EPOCH=$(echo "$EPOCH_INFO" | jq -r '.result.epoch')
SLOT_INDEX=$(echo "$EPOCH_INFO" | jq -r '.result.slotIndex')
SLOTS_IN_EPOCH=$(echo "$EPOCH_INFO" | jq -r '.result.slotsInEpoch')
BLOCK_HEIGHT=$(echo "$EPOCH_INFO" | jq -r '.result.blockHeight')
TRANSACTION_COUNT=$(echo "$EPOCH_INFO" | jq -r '.result.transactionCount // 0')

PERCENT_COMPLETE=$(awk "BEGIN {printf \"%.2f\", ($SLOT_INDEX / $SLOTS_IN_EPOCH) * 100}")
REMAINING_SLOTS=$((SLOTS_IN_EPOCH - SLOT_INDEX))

echo -e "  ${GREEN}✓${NC} Epoch: $EPOCH"
echo -e "  ${GREEN}✓${NC} Block Height: $BLOCK_HEIGHT"
echo -e "  ${GREEN}✓${NC} Slot Progress: $SLOT_INDEX / $SLOTS_IN_EPOCH ($PERCENT_COMPLETE%)"
echo -e "  ${GREEN}✓${NC} Remaining Slots: $REMAINING_SLOTS"
echo ""

# 5. Transaction Count
echo -e "${YELLOW}[4/8] Checking Transaction Count...${NC}"
TX_COUNT=$(rpc_call "getTransactionCount")
TX_TOTAL=$(echo "$TX_COUNT" | jq -r '.result')
echo -e "  ${GREEN}✓${NC} Total Transactions: $TX_TOTAL"
echo ""

# 6. Vote Accounts (Validator Status)
echo -e "${YELLOW}[5/8] Checking Validator Status...${NC}"
VOTE_ACCOUNTS=$(rpc_call "getVoteAccounts")

CURRENT_VALIDATORS=$(echo "$VOTE_ACCOUNTS" | jq -r '.result.current | length')
DELINQUENT_VALIDATORS=$(echo "$VOTE_ACCOUNTS" | jq -r '.result.delinquent | length')
TOTAL_ACTIVE_STAKE=$(echo "$VOTE_ACCOUNTS" | jq -r '[.result.current[].activatedStake] | add')
TOTAL_ACTIVE_STAKE_SOL=$(awk "BEGIN {printf \"%.2f\", $TOTAL_ACTIVE_STAKE / 1000000000}")

echo -e "  ${GREEN}✓${NC} Current Validators: $CURRENT_VALIDATORS"
echo -e "  ${GREEN}✓${NC} Delinquent Validators: $DELINQUENT_VALIDATORS"
echo -e "  ${GREEN}✓${NC} Total Active Stake: $TOTAL_ACTIVE_STAKE_SOL SOL"
echo ""

if [ "$DELINQUENT_VALIDATORS" -gt 0 ]; then
    echo -e "  ${RED}⚠${NC}  Warning: $DELINQUENT_VALIDATORS validators are delinquent"
fi

echo -e "${YELLOW}[6/8] Validator Performance Details...${NC}"
echo "$VOTE_ACCOUNTS" | jq -r '
.result.current[] |
"  Identity: \(.nodePubkey)\n  Vote Account: \(.votePubkey)\n  Last Vote: \(.lastVote)\n  Root Slot: \(.rootSlot)\n  Activated Stake: \((.activatedStake / 1000000000) | tostring) SOL\n  Commission: \(.commission)%\n  Epoch Credits: \(.epochCredits[-1][1] // 0)\n"
'

# 7. Cluster Nodes (Gossip)
echo -e "${YELLOW}[7/8] Checking Cluster Nodes (Gossip)...${NC}"
CLUSTER_NODES=$(rpc_call "getClusterNodes")
NODE_COUNT=$(echo "$CLUSTER_NODES" | jq -r '.result | length')
echo -e "  ${GREEN}✓${NC} Gossip Nodes: $NODE_COUNT"
echo ""

echo "$CLUSTER_NODES" | jq -r '
.result[] |
"  Node: \(.pubkey)\n  Gossip: \(.gossip // "N/A")\n  RPC: \(.rpc // "N/A")\n  Version: \(.version // "N/A")\n"
'

# 8. Block Production
echo -e "${YELLOW}[8/8] Checking Block Production...${NC}"
BLOCK_PRODUCTION=$(rpc_call "getBlockProduction")

if echo "$BLOCK_PRODUCTION" | jq -e '.result' > /dev/null 2>&1; then
    echo "$BLOCK_PRODUCTION" | jq -r '
    .result.value.byIdentity |
    to_entries[] |
    . as $entry |
    ($entry.value[1] / $entry.value[0] * 100) as $success_rate |
    ($entry.value[0] - $entry.value[1]) as $skipped |
    ($skipped / $entry.value[0] * 100) as $skip_rate |
    "  Validator: \($entry.key)\n  Leader Slots: \($entry.value[0])\n  Blocks Produced: \($entry.value[1])\n  Skipped: \($skipped)\n  Success Rate: \($success_rate | floor)%\n  Skip Rate: \($skip_rate | tonumber | . * 100 | round / 100)%\n"
    '
else
    echo -e "  ${YELLOW}⚠${NC}  Block production data not available"
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}✓ Health Check Complete${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Summary
if [ "$DELINQUENT_VALIDATORS" -eq 0 ] && [ "$CURRENT_VALIDATORS" -ge 3 ]; then
    echo -e "${GREEN}Network Status: HEALTHY ✓${NC}"
    echo "  • All validators active and voting"
    echo "  • Consensus forming ($CURRENT_VALIDATORS validators)"
    echo "  • Slot $CURRENT_SLOT progressing"
    exit 0
elif [ "$DELINQUENT_VALIDATORS" -gt 0 ]; then
    echo -e "${RED}Network Status: DEGRADED ⚠${NC}"
    echo "  • $DELINQUENT_VALIDATORS delinquent validators detected"
    exit 1
else
    echo -e "${YELLOW}Network Status: STARTING${NC}"
    echo "  • Waiting for more validators ($CURRENT_VALIDATORS active)"
    exit 0
fi
