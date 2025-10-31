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
echo -e "${BLUE}  SphereNet Testnet Program Upgrade${NC}"
echo -e "${BLUE}========================================${NC}\n"

# Check required variables
if [ -z "$PROGRAM_ID" ] || [ -z "$PROGRAM_SO" ] || [ -z "$UPGRADE_AUTHORITY" ] || [ -z "$PAYER" ]; then
    echo -e "${YELLOW}Error: Missing required environment variables${NC}"
    echo ""
    echo "Please create a .env file in the scripts/ directory."
    echo "You can copy .env.example and update it with your values:"
    echo ""
    echo "  cp scripts/.env.example scripts/.env"
    echo ""
    echo "Required variables:"
    echo "  - PROGRAM_ID"
    echo "  - PROGRAM_SO"
    echo "  - UPGRADE_AUTHORITY"
    echo "  - PAYER"
    echo ""
    echo "Optional variables:"
    echo "  - SPILL (defaults to payer if not set)"
    exit 1
fi

# Display configuration
echo -e "${YELLOW}Configuration:${NC}"
echo "  Program ID:         $PROGRAM_ID"
echo "  Program SO:         $PROGRAM_SO"
echo "  Upgrade Authority:  $UPGRADE_AUTHORITY"
echo "  Payer:              $PAYER"
if [ -n "$SPILL" ]; then
    echo "  Spill Account:      $SPILL"
fi
echo ""

# Change to the directory containing this script's parent
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Build the command
CMD="cargo run --quiet -- program upgrade \
    --program-id \"$PROGRAM_ID\" \
    --program-so \"$PROGRAM_SO\" \
    --upgrade-authority \"$UPGRADE_AUTHORITY\" \
    --payer \"$PAYER\""

# Add optional spill if provided
if [ -n "$SPILL" ]; then
    CMD="$CMD --spill \"$SPILL\""
fi

# Run the upgrade command
echo -e "${GREEN}Running upgrade command...${NC}\n"
eval "$CMD"

echo -e "\n${GREEN}✓ Upgrade complete!${NC}"
