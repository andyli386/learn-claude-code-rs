#!/bin/bash
# Quick setup script to update .env with correct model

echo "=== Updating .env file ==="
echo ""

if [ ! -f .env ]; then
    echo "Creating .env file from .env.example..."
    cp .env.example .env
fi

echo "Updating MODEL_NAME to claude-sonnet-4-5-20250929..."

# Update or add MODEL_NAME
if grep -q "^MODEL_NAME=" .env; then
    sed -i 's/^MODEL_NAME=.*/MODEL_NAME=claude-sonnet-4-5-20250929/' .env
else
    echo "MODEL_NAME=claude-sonnet-4-5-20250929" >> .env
fi

echo ""
echo "✓ Updated! Current .env content (with hidden token):"
echo ""
cat .env | sed 's/sk-[^=]*/sk-***HIDDEN***/g'
echo ""
echo "=== Setup Complete ==="
echo ""
echo "You can now run:"
echo "  cargo run -p v0_bash_agent"
echo "  cargo run -p v0_bash_agent sonnet"
echo "  cargo run -p v0_bash_agent opus"
