#!/bin/bash
set -e

# Change to script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Load environment variables
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SphereNet Testnet Program Deploy${NC}"
echo -e "${BLUE}========================================${NC}\n"

# Check required variables
if [ -z "$PROGRAM_SO" ] || [ -z "$PROGRAM_KEYPAIR" ] || [ -z "$UPGRADE_AUTHORITY" ] || [ -z "$PAYER" ]; then
    echo -e "${YELLOW}Error: Missing required environment variables${NC}"
    echo ""
    echo "Please create a .env file in the scripts/ directory."
    echo "You can copy .env.example and update it with your values:"
    echo ""
    echo "  cp scripts/.env.example scripts/.env"
    echo ""
    echo "Required variables:"
    echo "  - PROGRAM_SO"
    echo "  - PROGRAM_KEYPAIR"
    echo "  - UPGRADE_AUTHORITY"
    echo "  - PAYER"
    exit 1
fi

# Display configuration
echo -e "${YELLOW}Configuration:${NC}"
echo "  Program SO:         $PROGRAM_SO"
echo "  Program Keypair:    $PROGRAM_KEYPAIR"
echo "  Upgrade Authority:  $UPGRADE_AUTHORITY"
echo "  Payer:              $PAYER"
echo ""

# Change to the directory containing this script's parent
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Run the deploy command
echo -e "${GREEN}Running deploy command...${NC}\n"

cargo run --quiet -- program deploy \
    --program-so "$PROGRAM_SO" \
    --program-keypair "$PROGRAM_KEYPAIR" \
    --upgrade-authority "$UPGRADE_AUTHORITY" \
    --payer "$PAYER"

echo -e "\n${GREEN}✓ Deploy complete!${NC}"
