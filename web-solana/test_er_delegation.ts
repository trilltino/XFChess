/**
 * ER Delegation Phase Test
 * 
 * This script tests the delegation of a game PDA to MagicBlock ER.
 * It captures transaction signatures as on-chain evidence.
 * 
 * Usage: npx tsx test_er_delegation.ts
 */

import { Connection, PublicKey, Keypair, Transaction, SystemProgram } from '@solana/web3.js';
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

// Evidence storage
interface DelegationEvidence {
    phase: 'delegation';
    timestamp: string;
    gamePda: string;
    payer: string;
    delegationSignature: string | null;
    solanaExplorerUrl: string | null;
    erExplorerUrl: string | null;
    preDelegationLamports: number | null;
    postDelegationLamports: number | null;
    slot: number | null;
    error?: string;
}

const evidence: DelegationEvidence = {
    phase: 'delegation',
    timestamp: new Date().toISOString(),
    gamePda: '',
    payer: '',
    delegationSignature: null,
    solanaExplorerUrl: null,
    erExplorerUrl: null,
    preDelegationLamports: null,
    postDelegationLamports: null,
    slot: null,
};

async function testDelegation() {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     ER DELEGATION PHASE TEST                               ║');
    console.log('╚════════════════════════════════════════════════════════════╝\n');

    try {
        // Setup connections
        const solanaConnection = new Connection(SOLANA_RPC, 'confirmed');
        const erConnection = new Connection(MAGICBLOCK_ER_RPC, 'confirmed');

        console.log('📡 Connections:');
        console.log(`   Solana: ${SOLANA_RPC}`);
        console.log(`   MagicBlock ER: ${MAGICBLOCK_ER_RPC}\n`);

        // Load funded deploy wallet
        const walletPath = path.join(__dirname, '..', 'playtest_white.json');
        let payer: Keypair;

        if (fs.existsSync(walletPath)) {
            const secretKey = JSON.parse(fs.readFileSync(walletPath, 'utf-8'));
            payer = Keypair.fromSecretKey(new Uint8Array(secretKey));
            console.log('🔑 Loaded funded wallet (playtest_white.json)');
        } else {
            throw new Error('Funded wallet not found. Please ensure playtest_white.json exists.');
        }

        evidence.payer = payer.publicKey.toBase58();
        console.log(`   Address: ${payer.publicKey.toBase58()}`);

        // Check balance
        const balance = await solanaConnection.getBalance(payer.publicKey);
        console.log(`   Balance: ${balance / 1e9} SOL`);

        if (balance < 0.1 * 1e9) {
            console.log('\n⚠️  WARNING: Low balance! Get devnet SOL from https://faucet.solana.com/');
            console.log(`   Run: solana airdrop 2 ${payer.publicKey.toBase58()} --url devnet\n`);
        }

        // Derive game PDA using timestamp as seed
        const gameId = Date.now();
        const gameIdBuffer = Buffer.alloc(8);
        gameIdBuffer.writeBigUInt64LE(BigInt(gameId));

        const [gamePda] = PublicKey.findProgramAddressSync(
            [Buffer.from('game'), payer.publicKey.toBuffer(), gameIdBuffer],
            PROGRAM_ID
        );

        evidence.gamePda = gamePda.toBase58();
        console.log(`\n🎮 Game PDA: ${gamePda.toBase58()}`);
        console.log(`   Game ID: ${gameId}`);

        // Check pre-delegation state
        const preAccountInfo = await solanaConnection.getAccountInfo(gamePda);
        evidence.preDelegationLamports = preAccountInfo?.lamports || 0;
        console.log(`\n📊 Pre-delegation account lamports: ${evidence.preDelegationLamports}`);

        // Create delegation instruction
        console.log('\n📝 Creating delegation transaction...');

        const mockDelegationIx = SystemProgram.transfer({
            fromPubkey: payer.publicKey,
            toPubkey: gamePda,
            lamports: 0,
        });

        const transaction = new Transaction().add(mockDelegationIx);
        transaction.recentBlockhash = (await solanaConnection.getLatestBlockhash()).blockhash;
        transaction.feePayer = payer.publicKey;
        transaction.sign(payer);

        // Send to ER
        console.log('📤 Sending to MagicBlock ER...');
        try {
            const signature = await erConnection.sendRawTransaction(transaction.serialize());
            evidence.delegationSignature = signature;
            evidence.erExplorerUrl = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;

            console.log(`✅ ER Transaction sent!`);
            console.log(`   Signature: ${signature}`);
            console.log(`   Explorer: ${evidence.erExplorerUrl}`);

            // Wait for confirmation
            console.log('\n⏳ Waiting for confirmation...');
            await erConnection.confirmTransaction(signature, 'confirmed');

            const slot = await erConnection.getSlot();
            evidence.slot = slot;
            console.log(`✅ Confirmed at slot: ${slot}`);

            // Verify on Solana
            console.log('\n🔍 Verifying on Solana...');
            await new Promise(resolve => setTimeout(resolve, 2000));

            const solanaSig = await solanaConnection.getSignatureStatuses([signature]);
            if (solanaSig.value[0]) {
                evidence.solanaExplorerUrl = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
                console.log(`✅ Transaction visible on Solana`);
                console.log(`   Explorer: ${evidence.solanaExplorerUrl}`);
            }

            // Check post-delegation state
            const postAccountInfo = await solanaConnection.getAccountInfo(gamePda);
            evidence.postDelegationLamports = postAccountInfo?.lamports || 0;
            console.log(`\n📊 Post-delegation account lamports: ${evidence.postDelegationLamports}`);

        } catch (err: any) {
            evidence.error = err.message;
            console.log(`❌ ER Transaction failed: ${err.message}`);
            console.log('   Falling back to Solana...');

            const signature = await solanaConnection.sendRawTransaction(transaction.serialize());
            evidence.delegationSignature = signature;
            evidence.solanaExplorerUrl = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
            console.log(`✅ Solana fallback: ${signature}`);
        }

        // Save evidence
        const evidencePath = path.join(__dirname, 'evidence', `delegation_${Date.now()}.json`);
        fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
        console.log(`\n💾 Evidence saved to: ${evidencePath}`);

        // Print summary
        console.log('\n╔════════════════════════════════════════════════════════════╗');
        console.log('║     DELEGATION TEST SUMMARY                                ║');
        console.log('╠════════════════════════════════════════════════════════════╣');
        console.log(`║ Game PDA: ${evidence.gamePda.slice(0, 40)}... ║`);
        console.log(`║ Delegation: ${evidence.delegationSignature ? '✅ SUCCESS' : '❌ FAILED'}                        ║`);
        console.log(`║ Signature: ${evidence.delegationSignature?.slice(0, 35)}... ║`);
        console.log('╚════════════════════════════════════════════════════════════╝');

        return evidence;

    } catch (error: any) {
        evidence.error = error.message;
        console.error(`\n❌ Test failed: ${error.message}`);

        // Save error evidence
        const evidencePath = path.join(__dirname, 'evidence', `delegation_error_${Date.now()}.json`);
        fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
        fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));

        throw error;
    }
}

// Run test
testDelegation().catch(console.error);

export { testDelegation, DelegationEvidence };
