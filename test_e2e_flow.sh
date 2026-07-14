#!/bin/bash
set -e

echo "========================================="
echo "  SatsPath E2E Functionality Test Script "
echo "========================================="

echo "[1/7] Building the satspath binary..."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --bin satspath

BIN_PATH="$(pwd)/target/debug/satspath"
TEST_DIR="/tmp/satspath_test_env_$(date +%s)"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

echo ""
echo "[2/7] Initializing new temporary wallet at $TEST_DIR..."
$BIN_PATH wallet init

echo ""
echo "[3/7] Adding Payment Methods..."
$BIN_PATH wallet add-methods alice@satspath.dev \
    --lightning-address alice@getalby.com \
    --onchain-address bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4

echo ""
echo "[4/7] Showing Registered Profile (Signature Validation)..."
$BIN_PATH wallet show

echo ""
echo "[5/7] Testing Receive Preview (JSON) for 50,000 sats..."
$BIN_PATH wallet receive alice@satspath.dev 50000 --json

echo ""
echo "[6/7] Testing Pay/Quote Preview to Alice..."
$BIN_PATH pay alice@satspath.dev 50000

echo ""
echo "[7/7] Cleaning up temporary environment..."
rm -rf "$TEST_DIR"

echo "========================================="
echo "  All tests executed successfully! 🎉    "
echo "========================================="
