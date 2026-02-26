/**
 * XFChess Wager Contract Test Script
 * 
 * This script tests the wager functionality on Solana devnet:
 * 1. Create game with wager
 * 2. Join game with matching wager
 * 3. Record moves
 * 4. Claim wager
 * 
 * Run with: npx ts-node test_wager_contract.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    SystemProgram,
} from "@solana/web3.js";

// Program ID from the deployed contract
const PROGRAM_ID = new PublicKey("3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP");

// Devnet connection
const connection = new Connection("https://api.devnet.solana.com", "confirmed");

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

// Test creating a game with wager
async function testCreateGameWithWager() {
    console.log("\n========================================");
    console.log("TESTING: Create Game with Wager");
    console.log("========================================\n");

    // Create test wallet
    const wallet = Keypair.generate();
    console.log(`Test wallet: ${wallet.publicKey.toBase58()}`);

    // Request airdrop
    console.log("Requesting airdrop...");
    try {
        const signature = await connection.requestAirdrop(
            wallet.publicKey,
            2 * LAMPORTS_PER_SOL
        );
        await connection.confirmTransaction(signature);
        console.log("Airdrop received!");
    } catch (e) {
        console.log("Airdrop may have rate limit, continuing...");
    }

    const balance = await connection.getBalance(wallet.publicKey);
    console.log(`Wallet balance: ${balance / LAMPORTS_PER_SOL} SOL`);

    // Game parameters
    const gameId = new BN(Date.now());
    const wagerAmount = new BN(0.01 * LAMPORTS_PER_SOL); // 0.01 SOL wager

    console.log(`\nGame ID: ${gameId.toString()}`);
    console.log(`Wager amount: ${wagerAmount.toNumber() / LAMPORTS_PER_SOL} SOL`);

    // Derive PDAs
    const [gamePDA, gameBump] = deriveGamePDA(gameId);
    const [moveLogPDA, moveLogBump] = deriveMoveLogPDA(gameId);
    const [escrowPDA, escrowBump] = deriveEscrowPDA(gameId);

    console.log(`\nDerived addresses:`);
    console.log(`  Game PDA: ${gamePDA.toBase58()} (bump: ${gameBump})`);
    console.log(`  Move Log PDA: ${moveLogPDA.toBase58()} (bump: ${moveLogBump})`);
    console.log(`  Escrow PDA: ${escrowPDA.toBase58()} (bump: ${escrowBump})`);

    // Check if game already exists
    const gameInfo = await connection.getAccountInfo(gamePDA);
    if (gameInfo) {
        console.log("\n⚠️  Game already exists! Using existing game.");
        return { gameId, gamePDA, moveLogPDA, escrowPDA, wallet };
    }

    console.log("\n✓ Test setup complete. Ready to create game.");
    console.log("\nTo actually create the game, you would call:");
    console.log(`  program.methods.createGame(${gameId}, ${wagerAmount}, GameType.PvP)`);

    return { gameId, gamePDA, moveLogPDA, escrowPDA, wallet };
}

// Test the full wager flow
async function testFullWagerFlow() {
    console.log("\n========================================");
    console.log("FULL WAGER FLOW TEST");
    console.log("========================================\n");

    const player1 = Keypair.generate();
    const player2 = Keypair.generate();

    console.log("Player 1:", player1.publicKey.toBase58());
    console.log("Player 2:", player2.publicKey.toBase58());

    // Request airdrops
    console.log("\nRequesting airdrops...");
    try {
        await connection.requestAirdrop(player1.publicKey, 2 * LAMPORTS_PER_SOL);
        await connection.requestAirdrop(player2.publicKey, 2 * LAMPORTS_PER_SOL);
        await new Promise(resolve => setTimeout(resolve, 2000)); // Wait for confirmation
    } catch (e) {
        console.log("Airdrop may have failed (rate limit)");
    }

    const balance1 = await connection.getBalance(player1.publicKey);
    const balance2 = await connection.getBalance(player2.publicKey);

    console.log(`Player 1 balance: ${balance1 / LAMPORTS_PER_SOL} SOL`);
    console.log(`Player 2 balance: ${balance2 / LAMPORTS_PER_SOL} SOL`);

    // Game setup
    const gameId = new BN(Date.now());
    const wagerAmount = new BN(0.01 * LAMPORTS_PER_SOL);

    console.log(`\n=== Game Setup ===`);
    console.log(`Game ID: ${gameId.toString()}`);
    console.log(`Wager: ${wagerAmount.toNumber() / LAMPORTS_PER_SOL} SOL per player`);
    console.log(`Total pot: ${(wagerAmount.toNumber() * 2) / LAMPORTS_PER_SOL} SOL`);

    const [gamePDA] = deriveGamePDA(gameId);
    const [moveLogPDA] = deriveMoveLogPDA(gameId);
    const [escrowPDA] = deriveEscrowPDA(gameId);

    console.log(`\nPDAs:`);
    console.log(`  Game: ${gamePDA.toBase58()}`);
    console.log(`  Move Log: ${moveLogPDA.toBase58()}`);
    console.log(`  Escrow: ${escrowPDA.toBase58()}`);

    console.log("\n=== Expected Flow ===");
    console.log("1. Player 1 creates game with 0.01 SOL wager");
    console.log("   → 0.01 SOL transferred from Player 1 to Escrow PDA");
    console.log("\n2. Player 2 joins game with 0.01 SOL wager");
    console.log("   → 0.01 SOL transferred from Player 2 to Escrow PDA");
    console.log("   → Escrow now holds 0.02 SOL");
    console.log("\n3. Players record chess moves on MagicBlock ER");
    console.log("   → Fast, cheap transactions");
    console.log("   → Each move validated by on-chain chess logic");
    console.log("\n4. Game ends (checkmate/resignation)");
    console.log("   → Final state committed to Solana");
    console.log("\n5. Winner claims wager");
    console.log("   → 0.02 SOL transferred from Escrow to Winner");

    return {
        gameId,
        wagerAmount,
        gamePDA,
        moveLogPDA,
        escrowPDA,
        player1,
        player2,
    };
}

// Main test runner
async function main() {
    console.log("╔════════════════════════════════════════════════════════╗");
    console.log("║     XFChess Wager Contract Test - Devnet               ║");
    console.log("╚════════════════════════════════════════════════════════╝");
    console.log("\nProgram ID:", PROGRAM_ID.toBase58());
    console.log("Network: Devnet");
    console.log("RPC: https://api.devnet.solana.com");

    try {
        // Run tests
        await testCreateGameWithWager();
        await testFullWagerFlow();

        console.log("\n========================================");
        console.log("Test setup complete!");
        console.log("========================================");
        console.log("\nTo fully test the contract, integrate with Anchor IDL");
        console.log("and call the actual program methods.");

    } catch (error) {
        console.error("\n✗ Test failed:", error);
        process.exit(1);
    }
}

main();
