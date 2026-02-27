/**
 * On-Chain Evidence Verification
 * 
 * This script verifies all captured evidence against the Solana blockchain.
 * It checks transaction statuses, account states, and generates a verification report.
 * 
 * Usage: npx tsx verify_on_chain.ts [evidence_directory]
 */

import { Connection, PublicKey } from '@solana/web3.js';
import * as fs from 'fs';
import * as path from 'path';

const SOLANA_RPC = 'https://api.devnet.solana.com';
const MAGICBLOCK_ER_RPC = 'https://devnet-eu.magicblock.app';

interface VerificationResult {
    file: string;
    phase: string;
    timestamp: string;
    verified: boolean;
    checks: {
        signatureValid: boolean;
        onChainConfirmed: boolean;
        stateConsistent: boolean;
    };
    details: {
        signature?: string;
        slot?: number;
        confirmations?: string;
        error?: string;
    };
}

interface VerificationReport {
    generatedAt: string;
    rpcEndpoint: string;
    totalFiles: number;
    verified: number;
    failed: number;
    results: VerificationResult[];
}

async function verifyEvidence(evidenceDir?: string) {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     ON-CHAIN EVIDENCE VERIFICATION                         ║');
    console.log('╚════════════════════════════════════════════════════════════╝\n');

    const targetDir = evidenceDir || path.join(__dirname, 'evidence');

    if (!fs.existsSync(targetDir)) {
        console.log(`❌ Evidence directory not found: ${targetDir}`);
        console.log('   Run test scripts first to generate evidence.');
        process.exit(1);
    }

    // Setup connections
    const solanaConnection = new Connection(SOLANA_RPC, 'confirmed');
    const erConnection = new Connection(MAGICBLOCK_ER_RPC, 'confirmed');

    console.log('📡 RPC Endpoints:');
    console.log(`   Solana: ${SOLANA_RPC}`);
    console.log(`   MagicBlock ER: ${MAGICBLOCK_ER_RPC}`);
    console.log(`\n📁 Evidence Directory: ${targetDir}\n`);

    // Get all evidence files
    const files = fs.readdirSync(targetDir)
        .filter(f => f.endsWith('.json') && !f.includes('error') && !f.includes('verification'))
        .map(f => path.join(targetDir, f));

    if (files.length === 0) {
        console.log('❌ No evidence files found.');
        console.log('   Run test scripts first:');
        console.log('   - npx tsx test_er_delegation.ts');
        console.log('   - npx tsx test_er_gameplay.ts <game_pda>');
        console.log('   - npx tsx test_er_undelegation.ts <game_pda>');
        process.exit(1);
    }

    console.log(`🔍 Found ${files.length} evidence file(s)\n`);

    const report: VerificationReport = {
        generatedAt: new Date().toISOString(),
        rpcEndpoint: SOLANA_RPC,
        totalFiles: files.length,
        verified: 0,
        failed: 0,
        results: [],
    };

    for (const file of files.sort()) {
        const filename = path.basename(file);
        console.log(`─── Verifying: ${filename} ───`);

        const result: VerificationResult = {
            file: filename,
            phase: 'unknown',
            timestamp: '',
            verified: false,
            checks: {
                signatureValid: false,
                onChainConfirmed: false,
                stateConsistent: false,
            },
            details: {},
        };

        try {
            const evidence = JSON.parse(fs.readFileSync(file, 'utf-8'));
            result.phase = evidence.phase || 'unknown';
            result.timestamp = evidence.timestamp;

            // Verify based on phase
            switch (evidence.phase) {
                case 'delegation':
                    await verifyDelegation(evidence, result, solanaConnection, erConnection);
                    break;
                case 'gameplay':
                    await verifyGameplay(evidence, result, solanaConnection, erConnection);
                    break;
                case 'undelegation':
                    await verifyUndelegation(evidence, result, solanaConnection, erConnection);
                    break;
                default:
                    result.details.error = 'Unknown phase';
            }

            // Determine overall verification status
            result.verified = result.checks.signatureValid && result.checks.onChainConfirmed;

            if (result.verified) {
                report.verified++;
                console.log('   ✅ VERIFIED\n');
            } else {
                report.failed++;
                console.log('   ❌ FAILED\n');
            }

        } catch (error: any) {
            result.details.error = error.message;
            report.failed++;
            console.log(`   ❌ Error: ${error.message}\n`);
        }

        report.results.push(result);
    }

    // Save verification report
    const reportPath = path.join(targetDir, `verification_report_${Date.now()}.json`);
    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

    // Print summary
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║     VERIFICATION SUMMARY                                   ║');
    console.log('╠════════════════════════════════════════════════════════════╣');
    console.log(`║ Total Files: ${report.totalFiles.toString().padEnd(45)} ║`);
    console.log(`║ Verified: ${report.verified.toString().padEnd(48)} ║`);
    console.log(`║ Failed: ${report.failed.toString().padEnd(50)} ║`);
    console.log('╠════════════════════════════════════════════════════════════╣');
    console.log('║ Results:                                                   ║');

    for (const r of report.results) {
        const status = r.verified ? '✅' : '❌';
        const line = `║  ${status} ${r.phase.padEnd(12)} - ${r.file.slice(0, 35)}`;
        console.log(line.padEnd(59) + ' ║');
    }

    console.log('╠════════════════════════════════════════════════════════════╣');
    console.log(`║ Report saved to: ${path.basename(reportPath).padEnd(39)} ║`);
    console.log('╚════════════════════════════════════════════════════════════╝');

    return report;
}

async function verifyDelegation(
    evidence: any,
    result: VerificationResult,
    solanaConnection: Connection,
    erConnection: Connection
) {
    if (!evidence.delegationSignature) {
        result.details.error = 'No signature in evidence';
        return;
    }

    result.details.signature = evidence.delegationSignature;

    // Check on Solana
    console.log(`   Checking signature: ${evidence.delegationSignature.slice(0, 30)}...`);

    const status = await solanaConnection.getSignatureStatus(evidence.delegationSignature);
    if (status.value) {
        result.checks.signatureValid = true;
        result.checks.onChainConfirmed = status.value.confirmationStatus === 'confirmed';
        result.details.confirmations = status.value.confirmationStatus || 'unknown';
        result.details.slot = status.value.slot;

        console.log(`   Confirmations: ${result.details.confirmations}`);
        console.log(`   Slot: ${result.details.slot}`);
    } else {
        console.log('   ⚠️  Signature not found on Solana (may be ER-only)');
    }

    // Verify game PDA state
    if (evidence.gamePda) {
        const gamePda = new PublicKey(evidence.gamePda);
        const accountInfo = await solanaConnection.getAccountInfo(gamePda);

        if (accountInfo) {
            result.checks.stateConsistent =
                accountInfo.lamports === evidence.postDelegationLamports;
            console.log(`   Account state: ${result.checks.stateConsistent ? '✅' : '⚠️'}`);
        }
    }
}

async function verifyGameplay(
    evidence: any,
    result: VerificationResult,
    solanaConnection: Connection,
    erConnection: Connection
) {
    if (!evidence.moves || evidence.moves.length === 0) {
        result.details.error = 'No moves in evidence';
        return;
    }

    console.log(`   Verifying ${evidence.moves.length} moves...`);

    let confirmedMoves = 0;
    for (const move of evidence.moves) {
        if (move.signature) {
            const status = await solanaConnection.getSignatureStatus(move.signature);
            if (status.value?.confirmationStatus === 'confirmed') {
                confirmedMoves++;
            }
        }
    }

    result.checks.signatureValid = confirmedMoves > 0;
    result.checks.onChainConfirmed = confirmedMoves === evidence.moves.length;
    result.details.confirmations = `${confirmedMoves}/${evidence.moves.length} moves confirmed`;

    console.log(`   Confirmed: ${confirmedMoves}/${evidence.moves.length} moves`);
}

async function verifyUndelegation(
    evidence: any,
    result: VerificationResult,
    solanaConnection: Connection,
    erConnection: Connection
) {
    if (!evidence.undelegationSignature) {
        result.details.error = 'No signature in evidence';
        return;
    }

    result.details.signature = evidence.undelegationSignature;

    // Check on Solana
    console.log(`   Checking signature: ${evidence.undelegationSignature.slice(0, 30)}...`);

    const status = await solanaConnection.getSignatureStatus(evidence.undelegationSignature);
    if (status.value) {
        result.checks.signatureValid = true;
        result.checks.onChainConfirmed = status.value.confirmationStatus === 'confirmed';
        result.details.confirmations = status.value.confirmationStatus || 'unknown';
        result.details.slot = status.value.slot;

        console.log(`   Confirmations: ${result.details.confirmations}`);
        console.log(`   Slot: ${result.details.slot}`);
    }

    // Verify final state
    if (evidence.gamePda && evidence.postState) {
        const gamePda = new PublicKey(evidence.gamePda);
        const accountInfo = await solanaConnection.getAccountInfo(gamePda);

        if (accountInfo) {
            result.checks.stateConsistent =
                accountInfo.lamports === evidence.postState.lamports &&
                accountInfo.data.length === evidence.postState.dataSize;
            console.log(`   Final state: ${result.checks.stateConsistent ? '✅' : '⚠️'}`);
        }
    }
}

// Run verification
const evidenceDirArg = process.argv[2];
verifyEvidence(evidenceDirArg).catch(console.error);

export { verifyEvidence, VerificationReport, VerificationResult };
