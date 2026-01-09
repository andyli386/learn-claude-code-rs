#!/bin/bash
# Demo script for model selection feature

echo "=== v0_bash_agent Model Selection Demo ==="
echo ""

cd "$(dirname "$0")/../.."

echo "Test 1: Default model (interactive mode - will timeout after 2s)"
echo "-------------------------------------------------------"
timeout 2s cargo run -p v0_bash_agent 2>&1 | head -3 || true
echo ""

echo "Test 2: Sonnet model (one-shot task)"
echo "-------------------------------------------------------"
cargo run -p v0_bash_agent sonnet "echo 'Testing Sonnet model'" 2>&1 | grep -E "(Using model|Testing Sonnet)"
echo ""

echo "Test 3: Opus model (one-shot task)"
echo "-------------------------------------------------------"
cargo run -p v0_bash_agent opus "echo 'Testing Opus model'" 2>&1 | grep -E "(Using model|Testing Opus)"
echo ""

echo "Test 4: Task without model selection (uses default)"
echo "-------------------------------------------------------"
cargo run -p v0_bash_agent "echo 'Default model task'" 2>&1 | grep -E "(Using model|Default model)"
echo ""

echo "=== Demo Complete ==="
echo ""
echo "Usage examples:"
echo "  cargo run -p v0_bash_agent              # Interactive with default (Sonnet)"
echo "  cargo run -p v0_bash_agent sonnet       # Interactive with Sonnet"
echo "  cargo run -p v0_bash_agent opus         # Interactive with Opus"
echo "  cargo run -p v0_bash_agent 'task'       # One-shot with default"
echo "  cargo run -p v0_bash_agent sonnet 'task' # One-shot with Sonnet"
echo "  cargo run -p v0_bash_agent opus 'task'   # One-shot with Opus"
