/**
 * ER Gameplay Phase Test
 * 
 * This script tests move transactions through MagicBlock ER.
 * Captures signatures for each move as on-chain evidence.
 * 
 * Usage: npx tsx test_er_gameplay.ts <game_pda>
 */

import { Connection, PublicKey, Keypair, Transaction } from '@solana/web3.js';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Configuration
const SOLANA_RPC = 'https://api.devnet.solana.com';
const MAGICBLOCK_ER_RPC = 'https://devnet-eu.magicblock.app';
const PROGRAM_ID = new PublicKey('3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP');

// Evidence storage
interface MoveEvidence {
    moveNumber: number;
    uci: string;
    from: string;
    to: string;
    player: 'white' | 'black';
    signature: string;
    erConfirmed: boolean;
    solanaConfirmed: boolean;
    slot: number | null;
    latencyMs: number;
    error?: string;
}

interface GameplayEvidence {
    phase: 'gameplay';
    timestamp: string;
    gamePda: string;
    payer: string;
    moves: MoveEvidence[];
    totalMoves: number;
    successfulMoves: number;
    failedMoves: number;
    averageLatencyMs: number;
    finalGameState?: string;
}

const evidence: GameplayEvidence = {
    phase: 'gameplay',
    timestamp: new Date().toISOString(),
    gamePda: '',
    payer: '',
    moves: [],
    totalMoves: 0,
    successfulMoves: 0,
    failedMoves: 0,
    averageLatencyMs: 0,
};

// Test moves (standard chess opening)
const TEST_MOVES = [
    { uci: 'e2e4', player: 'white' as const },
    { uci: 'e7e5', player: 'black' as const },
    { uci: 'g1f3', player: 'white' as const },
    { uci: 'b8c6', player: 'black' as const },
    { uci: 'f1c4', player: 'white' as const },
    { uci: 'g8f6', player: 'black' as const },
];

async function testGameplay(gamePdaString?: string) {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     ER GAMEPLAY PHASE TEST                                 ║');
    console.log('╚════════════════════════════════════════════════════════════╝\n');

    try {
        // Setup connections
        const solanaConnection = new Connection(SOLANA_RPC, 'confirmed');
        const erConnection = new Connection(MAGICBLOCK_ER_RPC, 'confirmed');

        console.log('📡 Connections:');
        console.log(`   Solana: ${SOLANA_RPC}`);
        console.log(`   MagicBlock ER: ${MAGICBLOCK_ER_RPC}\n`);

        // Load test wallet
        const walletPath = path.join(__dirname, 'test_wallet.json');
        if (!fs.existsSync(walletPath)) {
            throw new Error('Test wallet not found. Run test_er_delegation.ts first.');
        }

        const secretKey = JSON.parse(fs.readFileSync(walletPath, 'utf-8'));
        const payer = Keypair.fromSecretKey(new Uint8Array(secretKey));
        evidence.payer = payer.publicKey.toBase58();
        console.log(`🔑 Payer: ${payer.publicKey.toBase58()}\n`);

        // Use provided game PDA or create new
        const gamePda = gamePdaString
            ? new PublicKey(gamePdaString)
            : PublicKey.findProgramAddressSync(
                [Buffer.from('game'), payer.publicKey.toBuffer(), new BN(1).toArrayLike(Buffer, 'le', 8)],
                PROGRAM_ID
            )[0];

        evidence.gamePda = gamePda.toBase58();
        console.log(`🎮 Game PDA: ${gamePda.toBase58()}\n`);

        // Verify game is delegated (check on ER)
        console.log('🔍 Checking delegation status on ER...');
        const erAccountInfo = await erConnection.getAccountInfo(gamePda);
        if (erAccountInfo) {
            console.log('✅ Game account found on ER');
            console.log(`   Lamports: ${erAccountInfo.lamports}`);
            console.log(`   Owner: ${erAccountInfo.owner.toBase58()}`);
        } else {
            console.log('⚠️  Game account not found on ER (may need delegation first)');
        }

        // Execute test moves
        console.log(`\n🎲 Executing ${TEST_MOVES.length} test moves...\n`);

        let totalLatency = 0;

        for (let i = 0; i < TEST_MOVES.length; i++) {
            const moveData = TEST_MOVES[i];
            console.log(`\n─── Move ${i + 1}: ${moveData.uci} (${moveData.player}) ───`);

            const moveEvidence: MoveEvidence = {
                moveNumber: i + 1,
                uci: moveData.uci,
                from: moveData.uci.slice(0, 2),
                to: moveData.uci.slice(2, 4),
                player: moveData.player,
                signature: '',
                erConfirmed: false,
                solanaConfirmed: false,
                slot: null,
                latencyMs: 0,
            };

            const startTime = Date.now();

            try {
                // Create move instruction (simplified - would use actual program instruction)
                const moveIx = {
                    keys: [
                        { pubkey: gamePda, isSigner: false, isWritable: true },
                        { pubkey: payer.publicKey, isSigner: true, isWritable: false },
                    ],
                    programId: PROGRAM_ID,
                    data: Buffer.from([1, ...Buffer.from(moveData.uci)]), // 1 = move instruction
                };

                const transaction = new Transaction().add(moveIx);
                transaction.recentBlockhash = (await erConnection.getLatestBlockhash()).blockhash;
                transaction.feePayer = payer.publicKey;
                transaction.sign(payer);

                // Send to ER
                console.log('📤 Sending to ER...');
                const signature = await erConnection.sendRawTransaction(transaction.serialize(), {
                    skipPreflight: false,
                    preflightCommitment: 'confirmed',
                });

                moveEvidence.signature = signature;
                console.log(`   Signature: ${signature}`);

                // Wait for ER confirmation
                console.log('⏳ Waiting for ER confirmation...');
                await erConnection.confirmTransaction(signature, 'confirmed');
                moveEvidence.erConfirmed = true;

                const erLatency = Date.now() - startTime;
                console.log(`   ER Latency: ${erLatency}ms ✅`);

                // Get slot
                const slot = await erConnection.getSlot();
                moveEvidence.slot = slot;

                // Check Solana for eventual consistency
                console.log('🔍 Checking Solana (eventual consistency)...');
                await new Promise(resolve => setTimeout(resolve, 1000));

                const solanaStatus = await solanaConnection.getSignatureStatuses([signature]);
                if (solanaStatus.value[0]?.confirmationStatus === 'confirmed') {
                    moveEvidence.solanaConfirmed = true;
                    console.log('   Solana confirmed ✅');
                } else {
                    console.log('   Solana pending ⏳');
                }

                const totalLatency = Date.now() - startTime;
                moveEvidence.latencyMs = totalLatency;
                console.log(`   Total Latency: ${totalLatency}ms`);

                evidence.successfulMoves++;
                totalLatency += totalLatency;

            } catch (error: any) {
                moveEvidence.error = error.message;
                console.log(`   ❌ Failed: ${error.message}`);
                evidence.failedMoves++;
            }

            evidence.moves.push(moveEvidence);
            evidence.totalMoves++;

            // Small delay between moves
            await new Promise(resolve => setTimeout(resolve, 500));
        }

        // Calculate average latency
        if (evidence.successfulMoves > 0) {
            evidence.averageLatencyMs = Math.round(totalLatency / evidence.successfulMoves);
        }

        // Save evidence
        const evidencePath = path.join(__dirname, 'evidence', `gameplay_${Date.now()}.json`);
        fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        // Print summary
        console.log('\n╔════════════════════════════════════════════════════════════╗');
        console.log('║     GAMEPLAY TEST SUMMARY                                  ║');
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log(`║ Total Moves: ${evidence.totalMoves.toString().padEnd(45)} ║`);
        console.log(`║ Successful: ${evidence.successfulMoves.toString().padEnd(46)} ║`);
        console.log(`║ Failed: ${evidence.failedMoves.toString().padEnd(50)} ║`);
        console.log(`║ Avg Latency: ${evidence.averageLatencyMs.toString().padEnd(44)}ms ║`);
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log('║ Evidence Files:                                            ║');
        evidence.moves.forEach((m, i) => {
            const line = `║  ${i + 1}. ${m.uci} - ${m.signature.slice(0, 30)}...`;
            console.log(line.padEnd(59) + ' ║');
        });
        console.log('╚════════════════════════════════════════════════════════════╝');
        console.log(`\n💾 Evidence saved to: ${evidencePath}`);

        return evidence;

    } catch (error: any) {
        evidence.error = error.message;
        console.error(`\n❌ Test failed: ${error.message}`);

        // Save error evidence
        const evidencePath = path.join(__dirname, 'evidence', `gameplay_error_${Date.now()}.json`);
        fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        throw error;
    }
}

// Run test
const gamePdaArg = process.argv[2];
testGameplay(gamePdaArg).catch(console.error);

export { testGameplay, GameplayEvidence, MoveEvidence };
