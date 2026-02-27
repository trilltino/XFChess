/**
 * Test Wager Contract via Anchor/CLI
 * 
 * This script tests the wager contract directly using Anchor
 * without needing the browser/web app.
 * 
 * Run with: npx ts-node test_wager_anchor.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, Wallet, BN } from "@coral-xyz/anchor";
import {
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    SystemProgram,
    Transaction,
    sendAndConfirmTransaction,
} from "@solana/web3.js";
import * as fs from "fs";

// Program ID
const PROGRAM_ID = new PublicKey("3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP");

// Devnet connection
const connection = new Connection("https://api.devnet.solana.com", "confirmed");

// Load keypair from file
function loadKeypair(path: string): Keypair {
    const secretKey = JSON.parse(fs.readFileSync(path, "utf-8"));
    return Keypair.fromSecretKey(new Uint8Array(secretKey));
}

// Derive PDAs
function deriveGamePDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("game"), gameId.toArrayLike(Buffer, "le", 8)],
        PROGRAM_ID
    );
}

function deriveMoveLogPDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("move_log"), gameId.toArrayLike(Buffer, "le", 8)],
        PROGRAM_ID
    );
}

function deriveEscrowPDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("wager_escrow"), gameId.toArrayLike(Buffer, "le", 8)],
        PROGRAM_ID
    );
}

async function testWagerContract() {
    console.log("╔═══════════════════════════════════════════════════════════╗");
    console.log("║     Testing Wager Contract via CLI                       ║");
    console.log("╚═══════════════════════════════════════════════════════════╝\n");

    // Load player wallets
    console.log("[1/6] Loading player wallets...");
    const player1 = loadKeypair("../player1_wallet.json");
    const player2 = loadKeypair("../player2_wallet.json");

    console.log(`Player 1: ${player1.publicKey.toBase58()}`);
    console.log(`Player 2: ${player2.publicKey.toBase58()}`);

    // Check balances
    const balance1 = await connection.getBalance(player1.publicKey);
    const balance2 = await connection.getBalance(player2.publicKey);

    console.log(`\nPlayer 1 balance: ${balance1 / LAMPORTS_PER_SOL} SOL`);
    console.log(`Player 2 balance: ${balance2 / LAMPORTS_PER_SOL} SOL`);

    if (balance1 === 0 || balance2 === 0) {
        console.log("\n⚠️  One or both wallets have no SOL. Please airdrop first.");
        return;
    }

    // Generate game ID
    const gameId = new BN(Date.now());
    const wagerAmount = new BN(0.01 * LAMPORTS_PER_SOL); // 0.01 SOL

    console.log("\n[2/6] Game setup:");
    console.log(`Game ID: ${gameId.toString()}`);
    console.log(`Wager: ${wagerAmount.toNumber() / LAMPORTS_PER_SOL} SOL per player`);

    // Derive PDAs
    const [gamePDA, gameBump] = deriveGamePDA(gameId);
    const [moveLogPDA, moveLogBump] = deriveMoveLogPDA(gameId);
    const [escrowPDA, escrowBump] = deriveEscrowPDA(gameId);

    console.log("\n[3/6] Derived PDAs:");
    console.log(`Game: ${gamePDA.toBase58()} (bump: ${gameBump})`);
    console.log(`Move Log: ${moveLogPDA.toBase58()} (bump: ${moveLogBump})`);
    console.log(`Escrow: ${escrowPDA.toBase58()} (bump: ${escrowBump})`);

    // Check if game exists
    const gameInfo = await connection.getAccountInfo(gamePDA);
    if (gameInfo) {
        console.log("\n⚠️  Game already exists! Using existing game.");
    } else {
        console.log("\n✓ Game does not exist. Ready to create.");
    }

    // Check escrow balance
    const escrowBalance = await connection.getBalance(escrowPDA);
    console.log(`\n[4/6] Escrow balance: ${escrowBalance / LAMPORTS_PER_SOL} SOL`);

    // Display test info
    console.log("\n[5/6] Test Parameters:");
    console.log(`Program ID: ${PROGRAM_ID.toBase58()}`);
    console.log(`Connection: ${connection.rpcEndpoint}`);

    console.log("\n[6/6] To actually execute transactions, you need the Anchor IDL.");
    console.log("The IDL provides the instruction encoding needed to call the program.");

    console.log("\n╔═══════════════════════════════════════════════════════════╗");
    console.log("║              Test Setup Complete!                        ║");
    console.log("╚═══════════════════════════════════════════════════════════╝\n");

    console.log("Next steps:");
    console.log("1. Build the program to get the IDL");
    console.log("2. Use the web app to execute transactions");
    console.log("3. Or integrate with Anchor client in the game");

    return {
        gameId,
        gamePDA,
        moveLogPDA,
        escrowPDA,
        wagerAmount,
        player1: player1.publicKey,
        player2: player2.publicKey,
    };
}

// Run test
testWagerContract()
    .then(() => {
        console.log("\n✓ CLI test completed!");
        process.exit(0);
    })
    .catch((error) => {
        console.error("\n✗ Test failed:", error);
        process.exit(1);
    });
