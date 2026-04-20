#!/bin/bash
# Generate Solana keys and update .env file on Hetzner server
# Run this on the server: bash generate-keys-on-server.sh

set -e

SENDGRID_KEY="SG.fake_key_for_now"

echo "=== XFChess Key Generation Script ==="
echo "This will generate Solana keys and update the .env file"
echo ""

# Install Solana CLI if not present
if ! command -v solana &> /dev/null; then
    echo "Installing Solana CLI..."
    sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"
    export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
fi

# Create keys directory
mkdir -p ~/xfchess-keys
cd ~/xfchess-keys

echo "Generating VPS Authority Key..."
solana-keygen new --outfile vps-authority.json --no-passphrase
VPS_KEY=$(solana-keygen recover 'prompt:?key=0' -o /dev/stdout < vps-authority.json)
VPS_PUBKEY=$(solana-keygen pubkey vps-authority.json)

echo "Generating KYC Authority Key..."
solana-keygen new --outfile kyc-authority.json --no-passphrase
KYC_KEY=$(solana-keygen recover 'prompt:?key=0' -o /dev/stdout < kyc-authority.json)
KYC_PUBKEY=$(solana-keygen pubkey kyc-authority.json)

echo "Generating Fee Payer Key..."
solana-keygen new --outfile fee-payer.json --no-passphrase
FEE_KEY=$(solana-keygen recover 'prompt:?key=0' -o /dev/stdout < fee-payer.json)
FEE_PUBKEY=$(solana-keygen pubkey fee-payer.json)

echo ""
echo "=== Keys Generated ==="
echo "VPS Authority Public: $VPS_PUBKEY"
echo "KYC Authority Public: $KYC_PUBKEY"
echo "Fee Payer Public: $FEE_PUBKEY"
echo ""

# Backup original .env if exists
if [ -f "/opt/xfchess/backend/.env" ]; then
    cp /opt/xfchess/backend/.env /opt/xfchess/backend/.env.backup
    echo "Backup created: /opt/xfchess/backend/.env.backup"
fi

# Update .env file
echo "Updating /opt/xfchess/backend/.env with new keys..."

# Read existing .env to preserve other values
if [ -f "/opt/xfchess/backend/.env" ]; then
    # Remove old key lines and add new ones
    sed -i '/^VPS_AUTHORITY_KEY=/d' /opt/xfchess/backend/.env
    sed -i '/^KYC_AUTHORITY_KEY=/d' /opt/xfchess/backend/.env
    sed -i '/^FEE_PAYER_KEYS=/d' /opt/xfchess/backend/.env
    sed -i '/^SENDGRID_API_KEY=/d' /opt/xfchess/backend/.env
fi

# Append new keys
cat >> /opt/xfchess/backend/.env << EOF

# Authority Keys (auto-generated)
VPS_AUTHORITY_KEY=$VPS_KEY
KYC_AUTHORITY_KEY=$KYC_KEY

# Fee Payer Key (auto-generated)
FEE_PAYER_KEYS=$FEE_KEY

# SendGrid API Key
SENDGRID_API_KEY=$SENDGRID_KEY
EOF

# Fix permissions
chmod 600 /opt/xfchess/backend/.env
chown xfchess:xfchess /opt/xfchess/backend/.env

echo ""
echo "=== .env Updated ==="
echo ""
echo "IMPORTANT: Fund the fee payer wallet with devnet SOL:"
echo "  solana airdrop 2 $FEE_PUBKEY --url https://api.devnet.solana.com"
echo ""
echo "Key files saved in ~/xfchess-keys/"
echo "Backup your keys securely!"
echo ""
echo "Restart the backend to apply changes:"
echo "  systemctl restart xfchess-backend"
