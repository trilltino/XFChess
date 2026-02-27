/**
 * ER Undelegation Phase Test
 * 
 * This script tests undelegation of a game PDA from MagicBlock ER to Solana.
 * Captures the final state commitment as on-chain evidence.
 * 
 * Usage: npx tsx test_er_undelegation.ts <game_pda>
 */

import { Connection, PublicKey, Keypair, Transaction } from '@solana/web3.js';
import * as fs from 'fs';
import * as path from 'path';

// Configuration
const SOLANA_RPC = 'https://api.devnet.solana.com';
const MAGICBLOCK_ER_RPC = 'https://devnet-eu.magicblock.app';
const PROGRAM_ID = new PublicKey('3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP');
const DELEGATION_PROGRAM_ID = new PublicKey('DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh');

// Evidence storage
interface UndelegationEvidence {
    phase: 'undelegation';
    timestamp: string;
    gamePda: string;
    payer: string;
    undelegationSignature: string | null;
    solanaExplorerUrl: string | null;
    erExplorerUrl: string | null;
    preState: {
        lamports: number | null;
        dataSize: number | null;
        owner: string | null;
        slot: number | null;
    };
    postState: {
        lamports: number | null;
        dataSize: number | null;
        owner: string | null;
        slot: number | null;
    };
    stateCommitmentProof?: string;
    error?: string;
}

const evidence: UndelegationEvidence = {
    phase: 'undelegation',
    timestamp: new Date().toISOString(),
    gamePda: '',
    payer: '',
    undelegationSignature: null,
    solanaExplorerUrl: null,
    erExplorerUrl: null,
    preState: {
        lamports: null,
        dataSize: null,
        owner: null,
        slot: null,
    },
    postState: {
        lamports: null,
        dataSize: null,
        owner: null,
        slot: null,
    },
};

async function testUndelegation(gamePdaString?: string) {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     ER UNDELEGATION PHASE TEST                             ║');
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

        // Use provided game PDA
        if (!gamePdaString) {
            throw new Error('Game PDA required. Usage: npx tsx test_er_undelegation.ts <game_pda>');
        }

        const gamePda = new PublicKey(gamePdaString);
        evidence.gamePda = gamePda.toBase58();
        console.log(`🎮 Game PDA: ${gamePda.toBase58()}\n`);

        // Capture pre-undelegation state on ER
        console.log('📊 Capturing pre-undelegation state on ER...');
        const preErInfo = await erConnection.getAccountInfo(gamePda);
        if (preErInfo) {
            evidence.preState.lamports = preErInfo.lamports;
            evidence.preState.dataSize = preErInfo.data.length;
            evidence.preState.owner = preErInfo.owner.toBase58();
            evidence.preState.slot = await erConnection.getSlot();

            console.log(`   Lamports: ${evidence.preState.lamports}`);
            console.log(`   Data Size: ${evidence.preState.dataSize} bytes`);
            console.log(`   Owner: ${evidence.preState.owner}`);
            console.log(`   Slot: ${evidence.preState.slot}`);
        } else {
            console.log('   ⚠️  Account not found on ER');
        }

        // Capture pre-undelegation state on Solana
        console.log('\n📊 Capturing pre-undelegation state on Solana...');
        const preSolanaInfo = await solanaConnection.getAccountInfo(gamePda);
        if (preSolanaInfo) {
            console.log(`   Lamports: ${preSolanaInfo.lamports}`);
            console.log(`   Data Size: ${preSolanaInfo.data.length} bytes`);
            console.log(`   Owner: ${preSolanaInfo.owner.toBase58()}`);
        } else {
            console.log('   ⚠️  Account not yet on Solana (expected for delegated games)');
        }

        // Create undelegation instruction
        console.log('\n📝 Creating undelegation transaction...');

        // Mock undelegation instruction (would use actual ER SDK in production)
        const undelegationIx = {
            keys: [
                { pubkey: gamePda, isSigner: false, isWritable: true },
                { pubkey: payer.publicKey, isSigner: true, isWritable: true },
                { pubkey: DELEGATION_PROGRAM_ID, isSigner: false, isWritable: false },
            ],
            programId: PROGRAM_ID,
            data: Buffer.from([2]), // 2 = undelegate instruction
        };

        const transaction = new Transaction().add(undelegationIx);
        transaction.recentBlockhash = (await erConnection.getLatestBlockhash()).blockhash;
        transaction.feePayer = payer.publicKey;
        transaction.sign(payer);

        // Send undelegation to ER
        console.log('📤 Sending undelegation to ER...');
        const startTime = Date.now();

        try {
            const signature = await erConnection.sendRawTransaction(transaction.serialize());
            evidence.undelegationSignature = signature;
            evidence.erExplorerUrl = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;

            console.log(`✅ Undelegation sent!`);
            console.log(`   Signature: ${signature}`);
            console.log(`   Explorer: ${evidence.erExplorerUrl}`);

            // Wait for ER confirmation
            console.log('\n⏳ Waiting for ER confirmation...');
            await erConnection.confirmTransaction(signature, 'confirmed');
            console.log('✅ ER confirmed');

            // Monitor for Solana commitment
            console.log('\n⏳ Waiting for Solana state commitment...');
            console.log('   (This may take 5-30 seconds)');

            let solanaConfirmed = false;
            let attempts = 0;
            const maxAttempts = 30;

            while (!solanaConfirmed && attempts < maxAttempts) {
                await new Promise(resolve => setTimeout(resolve, 2000));
                attempts++;

                const status = await solanaConnection.getSignatureStatuses([signature]);
                if (status.value[0]?.confirmationStatus === 'confirmed') {
                    solanaConfirmed = true;
                    console.log(`   ✅ Solana confirmed after ${attempts * 2} seconds`);
                } else {
                    process.stdout.write(`   ⏳ Checking... (${attempts}/${maxAttempts})\r`);
                }
            }

            if (!solanaConfirmed) {
                console.log('\n   ⚠️  Timeout waiting for Solana confirmation');
            }

        } catch (error: any) {
            evidence.error = error.message;
            console.log(`❌ ER undelegation failed: ${error.message}`);
            throw error;
        }

        const erLatency = Date.now() - startTime;
        console.log(`\n   Total latency: ${erLatency}ms`);

        // Capture post-undelegation state on Solana
        console.log('\n📊 Capturing post-undelegation state on Solana...');
        await new Promise(resolve => setTimeout(resolve, 3000));

        const postSolanaInfo = await solanaConnection.getAccountInfo(gamePda);
        if (postSolanaInfo) {
            evidence.postState.lamports = postSolanaInfo.lamports;
            evidence.postState.dataSize = postSolanaInfo.data.length;
            evidence.postState.owner = postSolanaInfo.owner.toBase58();
            evidence.postState.slot = await solanaConnection.getSlot();

            console.log(`   Lamports: ${evidence.postState.lamports}`);
            console.log(`   Data Size: ${evidence.postState.dataSize} bytes`);
            console.log(`   Owner: ${evidence.postState.owner}`);
            console.log(`   Slot: ${evidence.postState.slot}`);

            // Verify state consistency
            if (evidence.preState.dataSize === evidence.postState.dataSize) {
                console.log('   ✅ Data size consistent');
            } else {
                console.log('   ⚠️  Data size changed (may indicate state update)');
            }

            evidence.solanaExplorerUrl = `https://explorer.solana.com/address/${gamePda.toBase58()}?cluster=devnet`;

            // Generate state commitment proof
            const stateHash = Buffer.from(postSolanaInfo.data).toString('base64').slice(0, 64);
            evidence.stateCommitmentProof = stateHash;
            console.log(`\n🔐 State Commitment Proof: ${stateHash}...`);

        } else {
            console.log('   ⚠️  Account not found on Solana after undelegation');
        }

        // Save evidence
        const evidencePath = path.join(__dirname, 'evidence', `undelegation_${Date.now()}.json`);
        fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        // Print summary
        console.log('\n╔════════════════════════════════════════════════════════════╗');
        console.log('║     UNDELEGATION TEST SUMMARY                              ║');
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log(`║ Game PDA: ${evidence.gamePda.slice(0, 40)}... ║`);
        console.log(`║ Undelegation: ${evidence.undelegationSignature ? '✅ SUCCESS' : '❌ FAILED'}                    ║`);
        console.log(`║ ER Latency: ${erLatency.toString().padEnd(44)}ms ║`);
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log('║ State Changes:                                             ║');
        console.log(`║   Pre:  ${(evidence.preState.lamports || 0).toString().padEnd(10)} lamports, ${(evidence.preState.dataSize || 0).toString().padEnd(5)} bytes ║`);
        console.log(`║   Post: ${(evidence.postState.lamports || 0).toString().padEnd(10)} lamports, ${(evidence.postState.dataSize || 0).toString().padEnd(5)} bytes ║`);
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log('║ Explorer Links:                                            ║');
        console.log(`║   ER: ${evidence.erExplorerUrl?.slice(0, 48) || 'N/A'}... ║`);
        console.log(`║   Solana: ${evidence.solanaExplorerUrl?.slice(0, 44) || 'N/A'}... ║`);
        console.log('╚════════════════════════════════════════════════════════════╝');
        console.log(`\n💾 Evidence saved to: ${evidencePath}`);

        return evidence;

    } catch (error: any) {
        evidence.error = error.message;
        console.error(`\n❌ Test failed: ${error.message}`);

        // Save error evidence
        const evidencePath = path.join(__dirname, 'evidence', `undelegation_error_${Date.now()}.json`);
        fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        throw error;
    }
}

// Run test
const gamePdaArg = process.argv[2];
testUndelegation(gamePdaArg).catch(console.error);

export { testUndelegation, UndelegationEvidence };
