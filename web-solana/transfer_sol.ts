/**
 * Transfer SOL between wallets
 * 
 * Usage: npx tsx transfer_sol.ts <from_wallet.json> <to_address> <amount>
 */

import { Connection, Keypair, SystemProgram, Transaction, LAMPORTS_PER_SOL } from '@solana/web3.js';
import * as fs from 'fs';

const SOLANA_RPC = 'https://api.devnet.solana.com';

async function transferSOL() {
    const fromWalletPath = process.argv[2];
    const toAddress = process.argv[3];
    const amount = parseFloat(process.argv[4]);

    if (!fromWalletPath || !toAddress || !amount) {
        console.log('Usage: npx tsx transfer_sol.ts <from_wallet.json> <to_address> <amount_in_sol>');
        console.log('Example: npx tsx transfer_sol.ts playtest_white.json M5fTDBkDmRuVGk3SHzyQp9aZYaEMtxXi7dkR1jKYaYm 0.5');
        process.exit(1);
    }

    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     Transfer SOL                                           ║');
    console.log('╚════════════════════════════════════════════════════════════╝\n');

    try {
        // Load wallets
        const fromSecretKey = JSON.parse(fs.readFileSync(fromWalletPath, 'utf-8'));
        const fromKeypair = Keypair.fromSecretKey(new Uint8Array(fromSecretKey));
        const toPubkey = new (await import('@solana/web3.js')).PublicKey(toAddress);

        console.log(`📤 From: ${fromKeypair.publicKey.toBase58()}`);
        console.log(`📥 To: ${toAddress}`);
        console.log(`💰 Amount: ${amount} SOL\n`);

        // Connect to Solana
        const connection = new Connection(SOLANA_RPC, 'confirmed');
        console.log(`📡 Connected to: ${SOLANA_RPC}\n`);

        // Check balance
        const fromBalance = await connection.getBalance(fromKeypair.publicKey);
        console.log(`From balance: ${fromBalance / LAMPORTS_PER_SOL} SOL`);

        if (fromBalance < amount * LAMPORTS_PER_SOL) {
            console.log('❌ Insufficient balance!');
            process.exit(1);
        }

        // Create transfer
        const transaction = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: fromKeypair.publicKey,
                toPubkey: toPubkey,
                lamports: amount * LAMPORTS_PER_SOL,
            })
        );

        // Send transaction
        console.log('\n📝 Sending transaction...');
        const signature = await connection.sendTransaction(transaction, [fromKeypair]);
        console.log(`📤 Signature: ${signature}`);

        // Wait for confirmation
        console.log('⏳ Waiting for confirmation...');
        await connection.confirmTransaction(signature, 'confirmed');

        console.log('✅ Transfer complete!\n');

        // Check new balances
        const newFromBalance = await connection.getBalance(fromKeypair.publicKey);
        const newToBalance = await connection.getBalance(toPubkey);

        console.log('New balances:');
        console.log(`  From: ${newFromBalance / LAMPORTS_PER_SOL} SOL`);
        console.log(`  To: ${newToBalance / LAMPORTS_PER_SOL} SOL`);
        console.log(`\n🔗 Explorer: https://explorer.solana.com/tx/${signature}?cluster=devnet`);

    } catch (error: any) {
        console.error(`❌ Error: ${error.message}`);
        process.exit(1);
    }
}

transferSOL();
