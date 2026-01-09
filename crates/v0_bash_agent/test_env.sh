#!/bin/bash
# Test environment variable compatibility

echo "Testing v0_bash_agent environment variable support"
echo "==================================================="

# Test 1: Standard naming
echo ""
echo "Test 1: Standard naming (ANTHROPIC_API_KEY + ANTHROPIC_API_BASE)"
# ANTHROPIC_BASE_URL=https://xz.ai2api.dev/                                                                     
# ANTHROPIC_AUTH_TOKEN=sk-m68t4Fiq4clAyA4016PyhPORaK1p3icmPmf1CbWrqmAsYs8l 
export ANTHROPIC_API_KEY="sk-m68t4Fiq4clAyA4016PyhPORaK1p3icmPmf1CbWrqmAsYs8l"
export ANTHROPIC_API_BASE="https://xz.ai2api.dev/"
cargo run -p v0_bash_agent --bin v0_bash_agent "echo 'Environment test'" 2>&1 | head -5
unset ANTHROPIC_API_KEY ANTHROPIC_API_BASE

# Test 2: Alternative naming
echo ""
echo "Test 2: Alternative naming (ANTHROPIC_AUTH_TOKEN + ANTHROPIC_BASE_URL)"
export ANTHROPIC_AUTH_TOKEN="sk-m68t4Fiq4clAyA4016PyhPORaK1p3icmPmf1CbWrqmAsYs8l"
export ANTHROPIC_BASE_URL="https://xz.ai2api.dev/"
cargo run -p v0_bash_agent --bin v0_bash_agent "echo 'Environment test'" 2>&1 | head -5
unset ANTHROPIC_AUTH_TOKEN ANTHROPIC_BASE_URL

# Test 3: Missing variables
echo ""
echo "Test 3: Missing variables (should show error)"
cargo run -p v0_bash_agent --bin v0_bash_agent "echo 'test'" 2>&1 | head -5

echo ""
echo "==================================================="
echo "Test complete!"
