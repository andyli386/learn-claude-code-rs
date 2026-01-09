#!/bin/bash
# Quick test script for v0_bash_agent

echo "=== v0_bash_agent Quick Test ==="
echo ""
echo "Your configuration:"
echo "  API Token: ${ANTHROPIC_AUTH_TOKEN:0:20}..."
echo "  Base URL: $ANTHROPIC_BASE_URL"
echo "  Model: ${MODEL_NAME:-claude-sonnet-4-5-20250929}"
echo ""
echo "Running test: 'echo hello world'"
echo ""

cd "$(dirname "$0")/../.."
cargo run -p v0_bash_agent "echo hello world"
