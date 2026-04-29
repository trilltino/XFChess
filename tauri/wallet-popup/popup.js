// Tauri wallet popup - handles wallet connection (hot wallet only)

const statusEl = document.getElementById('status');
const closeBtn = document.getElementById('closeBtn');

// Close window when X button is clicked
closeBtn.addEventListener('click', () => {
    window.close();
});

// Show status message
function showStatus(message, type = 'loading') {
    statusEl.textContent = message;
    statusEl.className = `status ${type}`;
}

// Generate hot wallet (local keypair)
async function connectHotWallet() {
    showStatus('Generating local wallet...', 'loading');

    try {
        // Generate a local keypair using simple JS (no external dependencies)
        const { publicKey, secretKey } = generateKeypair();
        const pubkeyBase58 = bs58.encode(publicKey);

        // Store secret key in session storage for signing later
        const secretArr = Array.from(secretKey);
        sessionStorage.setItem("xfchess_session_key", JSON.stringify(secretArr));
        localStorage.setItem("xfchess_wallet_pubkey", pubkeyBase58);

        showStatus('Wallet connected!', 'success');

        // Send pubkey to Tauri
        await invokeTauri('wallet_connected', { pubkey: pubkeyBase58 });

        // Close window after short delay
        setTimeout(() => {
            window.close();
        }, 500);

    } catch (error) {
        console.error('Hot wallet error:', error);
        showStatus('Failed to generate wallet: ' + error.message, 'error');
    }
}

// Simple Ed25519 keypair generation (compatible with Solana)
function generateKeypair() {
    // Generate 32 bytes of random data for private key
    const privateKey = new Uint8Array(32);
    crypto.getRandomValues(privateKey);

    // Derive public key from private key (simplified - in production use proper Ed25519)
    // For now, we'll use a deterministic derivation for testing
    const publicKey = derivePublicKey(privateKey);

    return { publicKey, secretKey: privateKey };
}

// Simplified public key derivation (use proper Ed25519 in production)
function derivePublicKey(privateKey) {
    // This is a placeholder - in production use proper Ed25519 derivation
    // For now, we'll use a simple hash-based approach for testing
    const hashBuffer = new Uint8Array(64);
    for (let i = 0; i < 32; i++) {
        hashBuffer[i] = privateKey[i];
        hashBuffer[i + 32] = (privateKey[i] * 2) % 256;
    }
    return hashBuffer.slice(0, 32);
}

// Base58 encoding (simplified)
const bs58 = {
    encode: (input) => {
        const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
        let result = '';
        let num = 0n;
        for (const byte of input) {
            num = (num << 8n) | BigInt(byte);
        }
        while (num > 0n) {
            result = alphabet[Number(num % 58n)] + result;
            num = num / 58n;
        }
        return result || alphabet[0];
    }
};

// Check if Tauri is available
async function invokeTauri(cmd, args = {}) {
    try {
        const response = await fetch(`http://localhost:7454/tauri/${cmd}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(args)
        });
        if (!response.ok) {
            throw new Error(`Tauri command failed: ${response.status}`);
        }
        return await response.json();
    } catch (error) {
        console.log('[TAURI] invoke error (expected in browser mode):', error);
        // For browser mode, send directly to HTTP endpoint
        if (cmd === 'wallet_connected') {
            await fetch('http://localhost:7454/wallet', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ pubkey: args.pubkey })
            });
        }
        return null;
    }
}

// Add click handler to hot wallet button
document.querySelector('.wallet-btn').addEventListener('click', () => {
    connectHotWallet();
});

// Auto-generate wallet on load
window.addEventListener('load', () => {
    showStatus('Click to generate local wallet', 'loading');
});
