#!/bin/bash
# Fuzz test: Approve and execute a multisig proposal with random members
# Usage: ./approve_and_execute.sh <transaction_index>
#
# Tests
#
# Validator Whitelist (multisig authority)
#
# cargo run vw propose-authority 5pEoLGRx11W5LpKGZYhPmmViGAM5TkpsrLAqN4Fr2sNN --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run vw cancel-authority --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_1/id.json
#
# cargo run vw add BvX3ChSuEmcTWK3qDRoy4qPdsEAdLDx39H9UgyZDqjDu --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run vw update-start-epoch BvX3ChSuEmcTWK3qDRoy4qPdsEAdLDx39H9UgyZDqjDu --epoch 700 --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run vw update-end-epoch BvX3ChSuEmcTWK3qDRoy4qPdsEAdLDx39H9UgyZDqjDu --epoch 1000 --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run vw remove BvX3ChSuEmcTWK3qDRoy4qPdsEAdLDx39H9UgyZDqjDu --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
#
# cargo run vw add BvX3ChSuEmcTWK3qDRoy4qPdsEAdLDx39H9UgyZDqjDu --start-epoch 700 --end-epoch 1000 --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run vw remove BvX3ChSuEmcTWK3qDRoy4qPdsEAdLDx39H9UgyZDqjDu --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
#
# Program Whitelist (multisig authority)
# 
# cargo run pw propose-authority 5pEoLGRx11W5LpKGZYhPmmViGAM5TkpsrLAqN4Fr2sNN --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run pw cancel-authority --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
#
# cargo run pw add 2Wmxgkgny9VytPqcnY6SMTh9u6e5eHfibGLC9D4swakf --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
# cargo run pw remove 2Wmxgkgny9VytPqcnY6SMTh9u6e5eHfibGLC9D4swakf --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
#
# Program Deployment and Upgrade
#
# cargo run transfer --destination Cza2uAuWyoHfT5kfbJKrQKSkEF6XzzCS1XXsHQfXUqx6 --amount 0.1 --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json
#
# cargo run program deploy --program-so ../test-program/target/deploy/test_program.so --program-keypair ../test-program/target/deploy/test_program-keypair-1.json --upgrade-authority opt/testnet_multisig/member_0/id.json --payer opt/testnet_multisig/member_0/id.json --max-data-len 50000
#
# (solana check show and set-upgrade authority)
# solana program show Gkz9KvKTmkRujdFNKU6hxUx71LnS69BCC9ouvyt48qsG -u https://api.testnet.sphere.net
# solana program set-upgrade-authority Gkz9KvKTmkRujdFNKU6hxUx71LnS69BCC9ouvyt48qsG --new-upgrade-authority 2Wmxgkgny9VytPqcnY6SMTh9u6e5eHfibGLC9D4swakf --upgrade-authority opt/testnet_multisig/member_0/id.json --skip-new-upgrade-authority-signer-check -u https://api.testnet.sphere.net
#
# cargo run program upgrade --program-id Gkz9KvKTmkRujdFNKU6hxUx71LnS69BCC9ouvyt48qsG --program-so ../test-program/target/deploy/test_program.so --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json --payer opt/testnet_multisig/member_0/id.json
# cargo run program extend --program-id Gkz9KvKTmkRujdFNKU6hxUx71LnS69BCC9ouvyt48qsG --bytes 1000 --multisig 5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t --multisig-authority opt/testnet_multisig/member_0/id.json --payer opt/testnet_multisig/member_0/id.json

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <transaction_index>"
    exit 1
fi

TX_INDEX=$1
MULTISIG="5jV5k8rhrDv3NDTZ86uTdsgF1MSA642U1hagcvDPTz1t"
MEMBERS=(
    "opt/testnet_multisig/member_0/id.json"
    "opt/testnet_multisig/member_1/id.json"
    "opt/testnet_multisig/member_2/id.json"
)

echo "=============================================="
echo "Fuzz Testing: Transaction $TX_INDEX"
echo "=============================================="
echo ""

# Pick two random members for approvals (must be different)
# Then pick one random member for execution (can be any member including approvers)
APPROVER_1="${MEMBERS[$((RANDOM % 3))]}"
# Make sure approver 2 is different from approver 1
while true; do
    APPROVER_2="${MEMBERS[$((RANDOM % 3))]}"
    if [ "$APPROVER_2" != "$APPROVER_1" ]; then
        break
    fi
done
# Executor can be any member (including approvers)
EXECUTOR="${MEMBERS[$((RANDOM % 3))]}"

echo "Random selection:"
echo "  Approver 1: $APPROVER_1"
echo "  Approver 2: $APPROVER_2"
echo "  Executor:   $EXECUTOR"
echo ""

echo ">>> Approver 1 approving..."
cargo run multisig approve --multisig $MULTISIG --transaction-index $TX_INDEX --member "$APPROVER_1"

echo ""
echo "Waiting 2 seconds..."
sleep 2

echo ""
echo ">>> Approver 2 approving..."
cargo run multisig approve --multisig $MULTISIG --transaction-index $TX_INDEX --member "$APPROVER_2"

echo ""
echo "Waiting 2 seconds..."
sleep 2

echo ""
echo ">>> Executor executing..."
cargo run multisig execute --multisig $MULTISIG --transaction-index $TX_INDEX --member "$EXECUTOR"

echo ""
echo "✅ Done!"
