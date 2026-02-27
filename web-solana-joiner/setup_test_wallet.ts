/**
 * Setup Test Wallet with Devnet SOL
 * 
 * This script:
 * 1. Generates a new Solana keypair
 * 2. Requests devnet SOL airdrop
 * 3. Saves the keypair for testing
 * 
 * Run with: npx ts-node setup_test_wallet.ts
 */

import {
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
} from "@solana/web3.js";

// Devnet connection
const connection = new Connection("https://api.devnet.solana.com", "confirmed");

async function setupTestWallet() {
    console.log("╔═══════════════════════════════════════════════════════════╗");
    console.log("║     Setup Test Wallet with Devnet SOL                    ║");
    console.log("╚═══════════════════════════════════════════════════════════╝\n");

    // Generate new keypair
    console.log("[1/4] Generating new wallet...");
    const wallet = Keypair.generate();
    console.log(`  Public Key: ${wallet.publicKey.toBase58()}`);

    // Save secret key to file
    const fs = await import("fs");
    const keypairPath = `./test_wallet_${Date.now()}.json`;
    fs.writeFileSync(
        keypairPath,
        JSON.stringify(Array.from(wallet.secretKey))
    );
    console.log(`  Saved to: ${keypairPath}`);

    // Request airdrop
    console.log("\n[2/4] Requesting devnet SOL airdrop...");
    try {
        const signature = await connection.requestAirdrop(
            wallet.publicKey,
            2 * LAMPORTS_PER_SOL // 2 SOL
        );
        console.log(`  Airdrop requested: ${signature}`);

        // Wait for confirmation
        console.log("  Waiting for confirmation...");
        await connection.confirmTransaction(signature);
        console.log("  ✓ Airdrop confirmed!");
    } catch (error) {
        console.log("  ⚠ Airdrop may have failed (rate limit), continuing...");
    }

    // Check balance
    console.log("\n[3/4] Checking balance...");
    const balance = await connection.getBalance(wallet.publicKey);
    console.log(`  Balance: ${balance / LAMPORTS_PER_SOL} SOL`);

    if (balance === 0) {
        console.log("\n  ⚠ No SOL received. Try again later or use the faucet:");
        console.log("  https://faucet.solana.com/");
    }

    // Display wallet info
    console.log("\n[4/4] Wallet Info:");
    console.log(`  Public Key: ${wallet.publicKey.toBase58()}`);
    console.log(`  Secret Key: [${wallet.secretKey[0]}, ${wallet.secretKey[1]}, ...]`);
    console.log(`  Keypair File: ${keypairPath}`);

    console.log("\n╔═══════════════════════════════════════════════════════════╗");
    console.log("║                  Setup Complete!                          ║");
    console.log("╚═══════════════════════════════════════════════════════════╝\n");

    console.log("To use this wallet:");
    console.log("1. Import the keypair to Phantom/Solflare wallet");
    console.log("2. Or use it in your tests");
    console.log("3. Connect to the web app at http://localhost:5173");

    return {
        publicKey: wallet.publicKey,
        secretKey: wallet.secretKey,
        keypairPath,
        balance: balance / LAMPORTS_PER_SOL,
    };
}

// Run if called directly
if (require.main === module) {
    setupTestWallet()
        .then((result) => {
            console.log("\n✓ Test wallet ready!");
            process.exit(0);
        })
        .catch((error) => {
            console.error("\n✗ Failed:", error);
            process.exit(1);
        });
}

export { setupTestWallet };
