#!/bin/bash
# Run NixOS Sandbox API tests
#
# Usage:
#   ./run_tests.sh                              # Run all tests against localhost:8080
#   ./run_tests.sh http://myhost:8080           # Run against custom URL
#   ./run_tests.sh http://localhost:8080 -k browser     # Run only browser tests
#   ./run_tests.sh http://localhost:8080 -k multi_turn  # Run only multi-turn tests
#   ./run_tests.sh http://localhost:8080 --record       # Record screen during tests
#   ./run_tests.sh http://localhost:8080 --record --record-dir ./my-recordings

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
API_URL="${1:-http://localhost:8080}"
shift 2>/dev/null || true  # Remove first arg if it looks like a URL

# Install dependencies if needed
if ! python3 -c "import pytest, httpx, PIL" 2>/dev/null; then
    echo "Installing test dependencies..."
    pip install -r "$SCRIPT_DIR/requirements.txt"
fi

echo "============================================"
echo "NixOS Sandbox API Test Suite"
echo "============================================"
echo "API URL: $API_URL"

# Check for recording flag
RECORD_FLAG=""
for arg in "$@"; do
    if [ "$arg" == "--record" ]; then
        RECORD_FLAG="--record"
        echo "Recording: ENABLED"
        break
    fi
done

echo ""

# Check if API is reachable
echo "Checking API connectivity..."
if curl -s --connect-timeout 5 "$API_URL/health" > /dev/null; then
    echo "API is reachable!"
else
    echo "ERROR: Cannot reach API at $API_URL"
    echo "Make sure the sandbox is running:"
    echo "  docker compose up"
    exit 1
fi

echo ""
echo "Running tests..."
echo "============================================"

cd "$SCRIPT_DIR"
python3 -m pytest test_sandbox_api.py -v --api-url="$API_URL" "$@"

# Show recording location if enabled
if [ -n "$RECORD_FLAG" ]; then
    echo ""
    echo "============================================"
    echo "Recordings saved to: $SCRIPT_DIR/recordings/"
    echo "============================================"
fi
