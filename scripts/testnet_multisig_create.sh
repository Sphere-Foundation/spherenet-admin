#!/usr/bin/env bash
#
# Testnet multisig creation script
# Generates test keys and creates a multisig vault on SphereNet testnet
#

set -e

basedir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$basedir"

# Configuration
KEYDIR="${basedir}/opt/testnet_multisig"
PAYER="${HOME}/.config/solana/id.json"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Helper functions
info() {
    echo -e "${BLUE}${1}${NC}"
}

success() {
    echo -e "${GREEN}${1}${NC}"
}

err() {
    echo -e "${RED}${1}${NC}"
}

bold() {
    echo -e "${BOLD}${1}${NC}"
}

section() {
    echo ""
    bold "$1"
    echo ""
}

# Function to generate a multisig member keypair
generate_keypair() {
    local keydir="$1"
    local keyname="$2"

    mkdir -p "$keydir"

    solana-keygen new --no-bip39-passphrase -o "$keydir/id.json" --force > /dev/null

    # Generate pubkey.txt
    solana-keygen pubkey "$keydir/id.json" > "$keydir/pubkey.txt"

    info "  ✓ ${keyname}: $(cat $keydir/pubkey.txt)"
}

# Command handling
COMMAND=${1:-help}

case "$COMMAND" in
    "keygen")
        shift  # remove 'keygen' from args

        # Parse flags
        NUM_MEMBERS=3
        while [[ $# -gt 0 ]]; do
            case $1 in
                --members)
                    NUM_MEMBERS="$2"
                    shift 2
                    ;;
                *)
                    err "Unknown option: $1"
                    exit 1
                    ;;
            esac
        done

        # Check if keys already exist
        if [[ -d "${KEYDIR}" ]] && [[ -n "$(ls -A ${KEYDIR} 2>/dev/null)" ]]; then
            err "Error: Keys already exist in ${KEYDIR}/"
            echo ""
            echo "To regenerate:"
            echo "  1. Run: ./scripts/testnet_multisig_create.sh clean"
            echo "  2. Run keygen again"
            exit 1
        fi

        section "🔑 Generating ${NUM_MEMBERS} multisig member keypairs"

        # Generate member keys
        for i in $(seq 0 $((NUM_MEMBERS - 1))); do
            generate_keypair "${KEYDIR}/member_${i}" "Member ${i}"
        done

        # Generate create key (unique ID for multisig PDA)
        generate_keypair "${KEYDIR}/create_key" "Create Key"

        success "✅ Key generation complete!"
        echo "   Members:  ${NUM_MEMBERS}"
        echo "   Location: ${KEYDIR}"
        ;;

    "create")
        shift  # remove 'create' from args

        # Parse flags
        THRESHOLD=2
        while [[ $# -gt 0 ]]; do
            case $1 in
                --threshold)
                    THRESHOLD="$2"
                    shift 2
                    ;;
                *)
                    err "Unknown option: $1"
                    exit 1
                    ;;
            esac
        done

        # Check if keys exist
        if [[ ! -d "${KEYDIR}" ]] || [[ -z "$(ls -A ${KEYDIR} 2>/dev/null)" ]]; then
            err "Error: No keys found in ${KEYDIR}/"
            echo "Generate keys first: ./scripts/testnet_multisig_create.sh keygen"
            exit 1
        fi

        # Check if payer exists
        if [[ ! -f "${PAYER}" ]]; then
            err "Error: Payer keypair not found at ${PAYER}"
            exit 1
        fi

        section "🏗️  Creating multisig vault on SphereNet testnet"

        # Collect member pubkeys
        MEMBERS=()
        for memberdir in "${KEYDIR}"/member_*; do
            if [[ -d "$memberdir" ]]; then
                pubkey=$(cat "$memberdir/pubkey.txt")
                MEMBERS+=("$pubkey")
            fi
        done

        # Join members with commas
        MEMBERS_STR=$(IFS=,; echo "${MEMBERS[*]}")

        info "Configuration:"
        echo "  Members:   ${#MEMBERS[@]}"
        for i in "${!MEMBERS[@]}"; do
            echo "    [$((i+1))] ${MEMBERS[$i]}"
        done
        echo "  Threshold: ${THRESHOLD}/${#MEMBERS[@]}"
        echo ""

        # Run multisig create command
        cargo run --quiet -- multisig create \
            --members "${MEMBERS_STR}" \
            --threshold "${THRESHOLD}" \
            --create-key "${KEYDIR}/create_key/id.json" \
            --payer "${PAYER}"

        ;;

    "clean")
        section "🧹 Cleaning up test keys"

        if [[ -d "${KEYDIR}" ]]; then
            rm -rf "${KEYDIR}"
            success "✅ Removed ${KEYDIR}"
        else
            info "No keys to clean (${KEYDIR} doesn't exist)"
        fi
        ;;

    "help"|*)
        echo "Testnet Multisig Creation Tool"
        echo ""
        echo "Usage:"
        echo "  testnet_multisig_create.sh keygen [--members N]   Generate member keypairs (default: 3)"
        echo "  testnet_multisig_create.sh create [--threshold N] Create multisig vault (default: 2)"
        echo "  testnet_multisig_create.sh clean                  Remove generated test keys"
        echo ""
        echo "Example workflow:"
        echo "  1. Generate 3 member keys:"
        echo "     ./scripts/testnet_multisig_create.sh keygen"
        echo ""
        echo "  2. Create a 2-of-3 multisig:"
        echo "     ./scripts/testnet_multisig_create.sh create"
        echo ""
        echo "  3. Clean up when done:"
        echo "     ./scripts/testnet_multisig_create.sh clean"
        echo ""
        echo "Configuration:"
        echo "  Keys:  opt/testnet_multisig/"
        echo "  Payer: ${PAYER}"
        ;;
esac
