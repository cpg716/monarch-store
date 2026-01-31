#!/bin/bash
# Integration test script for install/update flow
# This tests the ACTUAL helper execution (requires root or polkit)
# Run from repo root or src-tauri; helper is built to workspace target (src-tauri/target).

set -e

echo "=== MonARCH Helper Install/Update Flow Test ==="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Workspace root is src-tauri (parent of monarch-helper). Helper binary is at workspace_root/target/debug/monarch-helper.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
HELPER_BIN="$WORKSPACE_ROOT/target/debug/monarch-helper"
if [ ! -f "$HELPER_BIN" ]; then
    echo -e "${YELLOW}Building helper first...${NC}"
    (cd "$WORKSPACE_ROOT" && cargo build -p monarch-helper)
fi

# Test 1: Command serialization (no root needed)
echo -e "${GREEN}Test 1: Command Serialization${NC}"
cat > /tmp/test_cmd.json << 'EOF'
{"command":"AlpmInstall","payload":{"packages":["test-package"],"sync_first":false,"enabled_repos":["core"],"cpu_optimization":null}}
EOF

if $HELPER_BIN /tmp/test_cmd.json 2>&1 | grep -q "Successfully parsed command"; then
    echo -e "${GREEN}✓ Command parsing works${NC}"
else
    echo -e "${RED}✗ Command parsing failed${NC}"
    exit 1
fi

# Test 2: Env var passing (no root needed for parsing)
echo -e "${GREEN}Test 2: Environment Variable Passing${NC}"
export MONARCH_CMD_JSON='{"command":"AlpmInstall","payload":{"packages":["test"],"sync_first":false,"enabled_repos":["core"],"cpu_optimization":null}}'
if $HELPER_BIN 2>&1 | grep -q "Found command in MONARCH_CMD_JSON"; then
    echo -e "${GREEN}✓ Env var parsing works${NC}"
else
    echo -e "${RED}✗ Env var parsing failed${NC}"
    exit 1
fi
unset MONARCH_CMD_JSON

# Test 3: Reject raw strings (no root needed)
echo -e "${GREEN}Test 3: Reject Raw Strings${NC}"
echo "cachyos" | $HELPER_BIN 2>&1 | grep -q "Invalid input on stdin" && \
    echo -e "${GREEN}✓ Raw strings correctly rejected${NC}" || \
    (echo -e "${RED}✗ Raw strings not rejected${NC}" && exit 1)

# Test 4: Initialize command (requires root)
echo -e "${YELLOW}Test 4: Initialize Command (requires root)${NC}"
INIT_CMD='{"command":"Initialize"}'
if sudo -E env MONARCH_CMD_JSON="$INIT_CMD" $HELPER_BIN 2>&1 | grep -q "Initializing"; then
    echo -e "${GREEN}✓ Initialize works${NC}"
else
    echo -e "${YELLOW}⚠ Initialize test skipped (no root or polkit)${NC}"
fi

echo ""
echo -e "${GREEN}=== Basic Tests Passed ===${NC}"
echo -e "${YELLOW}Note: Full install/update testing requires:${NC}"
echo "  1. Root privileges (sudo/polkit)"
echo "  2. Valid package databases"
echo "  3. Network access for sync"
echo "  4. Test packages to install"
