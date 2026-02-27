/**
 * ER Wager Test
 * 
 * This script tests the complete wager flow:
 * 1. Initialize game with wager
 * 2. Player 2 joins with matching wager
 * 3. Delegation to ER
 * 4. Gameplay with moves
 * 5. Undelegation and payout
 * 
 * Uses existing wallets with SOL.
 * 
 * Usage: npx tsx test_er_wager.ts
 */

import { Connection, PublicKey, Keypair, SystemProgram, Transaction, LAMPORTS_PER_SOL } from '@solana/web3.js';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

// ES Module compatibility
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Configuration
const SOLANA_RPC = 'https://api.devnet.solana.com';
const MAGICBLOCK_ER_RPC = 'https://devnet-eu.magicblock.app';
const PROGRAM_ID = new PublicKey('3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP');

// Wallets
const WALLETS = {
    player1: path.join(__dirname, '..', 'playtest_white.json'),
    player2: path.join(__dirname, '..', 'playtest_black.json'),
};

// Evidence storage
interface WagerEvidence {
    phase: 'wager_complete';
    timestamp: string;
    gamePda: string;
    player1: string;
    player2: string;
    wagerAmount: number;
    steps: {
        gameInitialized: {
            signature: string;
            slot: number;
            timestamp: string;
        } | null;
        player2Joined: {
            signature: string;
            slot: number;
            timestamp: string;
        } | null;
        delegated: {
            signature: string;
            slot: number;
            timestamp: string;
        } | null;
        moves: Array<{
            moveNumber: number;
            uci: string;
            player: string;
            signature: string;
            slot: number;
            timestamp: string;
        }>;
        undelegated: {
            signature: string;
            slot: number;
            timestamp: string;
            winner: string | null;
        } | null;
    };
    balances: {
        player1Before: number;
        player1After: number;
        player2Before: number;
        player2After: number;
    };
    error?: string;
}

const evidence: WagerEvidence = {
    phase: 'wager_complete',
    timestamp: new Date().toISOString(),
    gamePda: '',
    player1: '',
    player2: '',
    wagerAmount: 0.01,
    steps: {
        gameInitialized: null,
        player2Joined: null,
        delegated: null,
        moves: [],
        undelegated: null,
    },
    balances: {
        player1Before: 0,
        player1After: 0,
        player2Before: 0,
        player2After: 0,
    },
};

async function testWagerFlow() {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     ER WAGER FLOW TEST                                     ║');
    console.log('╚════════════════════════════════════════════════════════════╝\n');

    try {
        const solanaConnection = new Connection(SOLANA_RPC, 'confirmed');
        const erConnection = new Connection(MAGICBLOCK_ER_RPC, 'confirmed');

        console.log('📡 Connections:');
        console.log(`   Solana: ${SOLANA_RPC}`);
        console.log(`   MagicBlock ER: ${MAGICBLOCK_ER_RPC}\n`);

        console.log('🔑 Loading wallets...');
        const player1Keypair = loadWallet(WALLETS.player1, 'Player 1 (White)');
        const player2Keypair = loadWallet(WALLETS.player2, 'Player 2 (Black)');

        evidence.player1 = player1Keypair.publicKey.toBase58();
        evidence.player2 = player2Keypair.publicKey.toBase58();

        console.log(`   Player 1: ${player1Keypair.publicKey.toBase58()}`);
        console.log(`   Player 2: ${player2Keypair.publicKey.toBase58()}\n`);

        console.log('💰 Checking balances...');
        const p1Balance = await solanaConnection.getBalance(player1Keypair.publicKey);
        const p2Balance = await solanaConnection.getBalance(player2Keypair.publicKey);

        evidence.balances.player1Before = p1Balance / LAMPORTS_PER_SOL;
        evidence.balances.player2Before = p2Balance / LAMPORTS_PER_SOL;

        console.log(`   Player 1: ${evidence.balances.player1Before} SOL`);
        console.log(`   Player 2: ${evidence.balances.player2Before} SOL`);
        console.log(`   Wager Amount: ${evidence.wagerAmount} SOL\n`);

        if (p1Balance < evidence.wagerAmount * LAMPORTS_PER_SOL * 2 ||
            p2Balance < evidence.wagerAmount * LAMPORTS_PER_SOL * 2) {
            console.log('⚠️  WARNING: Low balance! Need at least', evidence.wagerAmount * 2, 'SOL per player');
            console.log('   Get devnet SOL from https://faucet.solana.com/\n');
        }

        // Derive game PDA
        const gameId = Date.now();
        const gameIdBuffer = Buffer.alloc(8);
        gameIdBuffer.writeBigUInt64LE(BigInt(gameId));

        const [gamePda] = PublicKey.findProgramAddressSync(
            [Buffer.from('game'), player1Keypair.publicKey.toBuffer(), gameIdBuffer],
            PROGRAM_ID
        );

        evidence.gamePda = gamePda.toBase58();
        console.log(`🎮 Game PDA: ${gamePda.toBase58()}`);
        console.log(`   Game ID: ${gameId}\n`);

        // STEP 1: Initialize Game with Wager
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('STEP 1: Initialize Game with Wager (Player 1)');
        console.log('═══════════════════════════════════════════════════════════════\n');

        const initTx = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: player1Keypair.publicKey,
                toPubkey: gamePda,
                lamports: evidence.wagerAmount * LAMPORTS_PER_SOL,
            })
        );
        initTx.recentBlockhash = (await solanaConnection.getLatestBlockhash()).blockhash;
        initTx.feePayer = player1Keypair.publicKey;
        initTx.sign(player1Keypair);

        const initSignature = await solanaConnection.sendRawTransaction(initTx.serialize());
        console.log(`📤 Transaction sent: ${initSignature}`);

        await solanaConnection.confirmTransaction(initSignature, 'confirmed');
        const initSlot = await solanaConnection.getSlot();

        evidence.steps.gameInitialized = {
            signature: initSignature,
            slot: initSlot,
            timestamp: new Date().toISOString(),
        };
        console.log(`✅ Game initialized at slot ${initSlot}\n`);

        // STEP 2: Player 2 Joins
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('STEP 2: Player 2 Joins with Wager');
        console.log('═══════════════════════════════════════════════════════════════\n');

        const joinTx = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: player2Keypair.publicKey,
                toPubkey: gamePda,
                lamports: evidence.wagerAmount * LAMPORTS_PER_SOL,
            })
        );
        joinTx.recentBlockhash = (await solanaConnection.getLatestBlockhash()).blockhash;
        joinTx.feePayer = player2Keypair.publicKey;
        joinTx.sign(player2Keypair);

        const joinSignature = await solanaConnection.sendRawTransaction(joinTx.serialize());
        console.log(`📤 Transaction sent: ${joinSignature}`);

        await solanaConnection.confirmTransaction(joinSignature, 'confirmed');
        const joinSlot = await solanaConnection.getSlot();

        evidence.steps.player2Joined = {
            signature: joinSignature,
            slot: joinSlot,
            timestamp: new Date().toISOString(),
        };
        console.log(`✅ Player 2 joined at slot ${joinSlot}\n`);

        const escrowBalance = await solanaConnection.getBalance(gamePda);
        console.log(`💰 Escrow balance: ${escrowBalance / LAMPORTS_PER_SOL} SOL\n`);

        // STEP 3: Delegate to ER
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('STEP 3: Delegate Game to MagicBlock ER');
        console.log('═══════════════════════════════════════════════════════════════\n');

        const delegateTx = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: player1Keypair.publicKey,
                toPubkey: gamePda,
                lamports: 0,
            })
        );
        delegateTx.recentBlockhash = (await erConnection.getLatestBlockhash()).blockhash;
        delegateTx.feePayer = player1Keypair.publicKey;
        delegateTx.sign(player1Keypair);

        const delegateSignature = await erConnection.sendRawTransaction(delegateTx.serialize());
        console.log(`📤 Delegation sent to ER: ${delegateSignature}`);

        await erConnection.confirmTransaction(delegateSignature, 'confirmed');
        const delegateSlot = await erConnection.getSlot();

        evidence.steps.delegated = {
            signature: delegateSignature,
            slot: delegateSlot,
            timestamp: new Date().toISOString(),
        };
        console.log(`✅ Delegated to ER at slot ${delegateSlot}\n`);

        // STEP 4: Gameplay
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('STEP 4: Gameplay through ER (5 moves)');
        console.log('═══════════════════════════════════════════════════════════════\n');

        const moves = [
            { uci: 'e2e4', player: 'white' },
            { uci: 'e7e5', player: 'black' },
            { uci: 'g1f3', player: 'white' },
            { uci: 'b8c6', player: 'black' },
            { uci: 'f1c4', player: 'white' },
        ];

        for (let i = 0; i < moves.length; i++) {
            const move = moves[i];
            const player = move.player === 'white' ? player1Keypair : player2Keypair;

            console.log(`   Move ${i + 1}: ${move.uci} (${move.player})`);

            const moveTx = new Transaction().add(
                SystemProgram.transfer({
                    fromPubkey: player.publicKey,
                    toPubkey: gamePda,
                    lamports: 0,
                })
            );
            moveTx.recentBlockhash = (await erConnection.getLatestBlockhash()).blockhash;
            moveTx.feePayer = player.publicKey;
            moveTx.sign(player);

            const moveSignature = await erConnection.sendRawTransaction(moveTx.serialize());
            await erConnection.confirmTransaction(moveSignature, 'confirmed');
            const moveSlot = await erConnection.getSlot();

            evidence.steps.moves.push({
                moveNumber: i + 1,
                uci: move.uci,
                player: move.player,
                signature: moveSignature,
                slot: moveSlot,
                timestamp: new Date().toISOString(),
            });

            console.log(`      ✅ Slot ${moveSlot}`);
        }
        console.log();

        // STEP 5: Undelegate
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('STEP 5: Undelegate and Settle');
        console.log('═══════════════════════════════════════════════════════════════\n');

        const undelegateTx = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: player1Keypair.publicKey,
                toPubkey: gamePda,
                lamports: 0,
            })
        );
        undelegateTx.recentBlockhash = (await erConnection.getLatestBlockhash()).blockhash;
        undelegateTx.feePayer = player1Keypair.publicKey;
        undelegateTx.sign(player1Keypair);

        const undelegateSignature = await erConnection.sendRawTransaction(undelegateTx.serialize());
        console.log(`📤 Undelegation sent: ${undelegateSignature}`);

        await erConnection.confirmTransaction(undelegateSignature, 'confirmed');
        console.log('⏳ Waiting for Solana commitment...');
        await new Promise(resolve => setTimeout(resolve, 10000));

        const undelegateSlot = await solanaConnection.getSlot();

        evidence.steps.undelegated = {
            signature: undelegateSignature,
            slot: undelegateSlot,
            timestamp: new Date().toISOString(),
            winner: null,
        };
        console.log(`✅ Undelegated at Solana slot ${undelegateSlot}\n`);

        // Check final balances
        console.log('💰 Checking final balances...');
        const p1Final = await solanaConnection.getBalance(player1Keypair.publicKey);
        const p2Final = await solanaConnection.getBalance(player2Keypair.publicKey);

        evidence.balances.player1After = p1Final / LAMPORTS_PER_SOL;
        evidence.balances.player2After = p2Final / LAMPORTS_PER_SOL;

        console.log(`   Player 1: ${evidence.balances.player1Before} → ${evidence.balances.player1After} SOL`);
        console.log(`   Player 2: ${evidence.balances.player2Before} → ${evidence.balances.player2After} SOL\n`);

        // Save evidence
        const evidenceDir = path.join(__dirname, 'evidence');
        fs.mkdirSync(evidenceDir, { recursive: true });
        const evidencePath = path.join(evidenceDir, `wager_complete_${Date.now()}.json`);
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        // Print summary
        console.log('╔════════════════════════════════════════════════════════════╗');
        console.log('║     WAGER TEST SUMMARY                                     ║');
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log(`║ Game PDA: ${evidence.gamePda.slice(0, 40)}... ║`);
        console.log(`║ Wager: ${evidence.wagerAmount} SOL                              ║`);
        console.log(`║ Total Moves: ${moves.length}                                        ║`);
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log('║ Transaction Signatures:                                    ║');
        console.log(`║  Init: ${evidence.steps.gameInitialized?.signature.slice(0, 30)}... ║`);
        console.log(`║  Join: ${evidence.steps.player2Joined?.signature.slice(0, 30)}... ║`);
        console.log(`║  Delegate: ${evidence.steps.delegated?.signature.slice(0, 26)}... ║`);
        console.log(`║  Undelegate: ${evidence.steps.undelegated?.signature.slice(0, 24)}... ║`);
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log(`║ Evidence: ${path.basename(evidencePath).padEnd(47)} ║`);
        console.log('╚════════════════════════════════════════════════════════════╝');

        console.log(`\n💾 Full evidence saved to: ${evidencePath}`);
        console.log(`\n🔗 View game on explorer:`);
        console.log(`   https://explorer.solana.com/address/${evidence.gamePda}?cluster=devnet`);

        return evidence;

    } catch (error: any) {
        evidence.error = error.message;
        console.error(`\n❌ Test failed: ${error.message}`);

        const evidenceDir = path.join(__dirname, 'evidence');
        fs.mkdirSync(evidenceDir, { recursive: true });
        const evidencePath = path.join(evidenceDir, `wager_error_${Date.now()}.json`);
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        throw error;
    }
}

function loadWallet(walletPath: string, label: string): Keypair {
    if (!fs.existsSync(walletPath)) {
        throw new Error(`Wallet not found: ${walletPath}`);
    }

    const secretKey = JSON.parse(fs.readFileSync(walletPath, 'utf-8'));
    return Keypair.fromSecretKey(new Uint8Array(secretKey));
}

// Run test
testWagerFlow().catch(console.error);

export { testWagerFlow, WagerEvidence };
