/**
 * CLI Test Script for Create Game Transaction
 * 
 * This script tests the create_game instruction on Solana devnet.
 * Run with: npx ts-node test_create_game.ts
 */

import * as anchor from '@coral-xyz/anchor'
import { Program, BN } from '@coral-xyz/anchor'
import {
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    SystemProgram,
} from '@solana/web3.js'
import * as fs from 'fs'
import * as path from 'path'

// Program ID
const PROGRAM_ID = new PublicKey('3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP')

// Devnet connection
const connection = new Connection('https://api.devnet.solana.com', 'confirmed')

// Load IDL
const idlPath = path.join(__dirname, 'src', 'idl', 'xfchess_game.json')
const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'))

// Derive PDAs
function deriveGamePDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [Buffer.from('game'), gameId.toArrayLike(Buffer, 'le', 8)],
        PROGRAM_ID
    )
}

function deriveMoveLogPDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [Buffer.from('move_log'), gameId.toArrayLike(Buffer, 'le', 8)],
        PROGRAM_ID
    )
}

function deriveEscrowPDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [Buffer.from('escrow'), gameId.toArrayLike(Buffer, 'le', 8)],
        PROGRAM_ID
    )
}

async function testCreateGame() {
    console.log('╔═══════════════════════════════════════════════════════════╗')
    console.log('║     XFChess - Create Game Transaction Test               ║')
    console.log('╚═══════════════════════════════════════════════════════════╝\n')

    // Generate a test wallet
    console.log('[1/6] Generating test wallet...')
    const wallet = Keypair.generate()
    console.log(`  Wallet: ${wallet.publicKey.toBase58()}`)

    // Request airdrop
    console.log('\n[2/6] Requesting airdrop (2 SOL)...')
    try {
        const signature = await connection.requestAirdrop(
            wallet.publicKey,
            2 * LAMPORTS_PER_SOL
        )
        await connection.confirmTransaction(signature)
        console.log('  ✓ Airdrop received')
    } catch (e) {
        console.log('  ⚠ Airdrop may have rate limit, continuing...')
    }

    // Check balance
    const balance = await connection.getBalance(wallet.publicKey)
    console.log(`  Balance: ${balance / LAMPORTS_PER_SOL} SOL`)

    if (balance < 0.1 * LAMPORTS_PER_SOL) {
        console.log('\n  ✗ Insufficient balance. Please fund the wallet manually.')
        console.log(`  solana airdrop 2 ${wallet.publicKey.toBase58()} --url devnet`)
        return
    }

    // Setup Anchor provider
    console.log('\n[3/6] Setting up Anchor provider...')
    const provider = new anchor.AnchorProvider(
        connection,
        {
            publicKey: wallet.publicKey,
            signTransaction: async (tx) => {
                tx.partialSign(wallet)
                return tx
            },
            signAllTransactions: async (txs) => {
                txs.forEach(tx => tx.partialSign(wallet))
                return txs
            },
        } as any,
        { commitment: 'confirmed' }
    )
    anchor.setProvider(provider)

    // Create program instance
    const program = new Program(idl, provider)
    console.log('  ✓ Program initialized')

    // Generate game parameters
    console.log('\n[4/6] Preparing game parameters...')
    const gameId = new BN(Date.now())
    const wagerAmount = new BN(0.01 * LAMPORTS_PER_SOL) // 0.01 SOL
    // @ts-ignore - Anchor enum workaround: lowercase variants required for serialization
    const gameType = { pvp: {} }

    console.log(`  Game ID: ${gameId.toString()}`)
    console.log(`  Wager: ${wagerAmount.toNumber() / LAMPORTS_PER_SOL} SOL`)
    console.log(`  Type: PvP`)

    // Derive PDAs
    const [gamePDA, gameBump] = deriveGamePDA(gameId)
    const [moveLogPDA, moveLogBump] = deriveMoveLogPDA(gameId)
    const [escrowPDA, escrowBump] = deriveEscrowPDA(gameId)

    console.log('\n[5/6] Derived PDAs:')
    console.log(`  Game: ${gamePDA.toBase58()} (bump: ${gameBump})`)
    console.log(`  Move Log: ${moveLogPDA.toBase58()} (bump: ${moveLogBump})`)
    console.log(`  Escrow: ${escrowPDA.toBase58()} (bump: ${escrowBump})`)

    // Check if game already exists
    const gameInfo = await connection.getAccountInfo(gamePDA)
    if (gameInfo) {
        console.log('\n  ⚠ Game already exists! Using new game ID...')
        return testCreateGame() // Retry with new timestamp
    }

    // Execute create_game transaction
    console.log('\n[6/6] Executing create_game transaction...')
    try {
        const txSignature = await program.methods
            .createGame(gameId, wagerAmount, gameType)
            .accounts({
                game: gamePDA,
                moveLog: moveLogPDA,
                escrowPda: escrowPDA,
                player: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc()

        console.log('  ✓ Transaction successful!')
        console.log(`  Signature: ${txSignature}`)
        console.log(`  Explorer: https://explorer.solana.com/tx/${txSignature}?cluster=devnet`)

        // Verify game account
        console.log('\n[Verification] Checking created accounts...')
        await new Promise(resolve => setTimeout(resolve, 2000)) // Wait for confirmation

        const gameAccount = await program.account.game.fetch(gamePDA)
        console.log('  ✓ Game account created:')
        console.log(`    - ID: ${(gameAccount.gameId as BN).toString()}`)
        console.log(`    - White: ${(gameAccount.white as PublicKey).toBase58()}`)
        console.log(`    - Wager: ${(gameAccount.wagerAmount as BN).toNumber() / LAMPORTS_PER_SOL} SOL`)
        console.log(`    - Status: ${JSON.stringify(gameAccount.status)}`)

        // Check escrow balance
        const escrowBalance = await connection.getBalance(escrowPDA)
        console.log(`\n  ✓ Escrow balance: ${escrowBalance / LAMPORTS_PER_SOL} SOL`)

        console.log('\n╔═══════════════════════════════════════════════════════════╗')
        console.log('║              ✓ Test Completed Successfully!               ║')
        console.log('╚═══════════════════════════════════════════════════════════╝\n')

        return {
            gameId: gameId.toString(),
            gamePDA: gamePDA.toBase58(),
            signature: txSignature,
        }
    } catch (err) {
        console.error('\n  ✗ Transaction failed:', err)
        throw err
    }
}

// Run test
testCreateGame()
    .then((result) => {
        console.log('Test result:', result)
        process.exit(0)
    })
    .catch((error) => {
        console.error('Test failed:', error)
        process.exit(1)
    })
