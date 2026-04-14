/**
 * MagicBlock Ephemeral Rollups Client
 * 
 * Implements dual-connection architecture from MagicBlock Dev Skill:
 * - Base layer connection for initialization and delegation
 * - Ephemeral rollup connection for operations on delegated accounts
 */

import { AnchorProvider, Program, type Idl } from '@coral-xyz/anchor';
import { Connection, PublicKey, type Commitment } from '@solana/web3.js';
import {
  DELEGATION_PROGRAM_ID,
  GetCommitmentSignature,
} from '@magicblock-labs/ephemeral-rollups-sdk';
import idl from './xfchess_game.json';

// Program IDs from MagicBlock Dev Skill
export const MAGIC_PROGRAM_ID = new PublicKey('Magic11111111111111111111111111111111111111');
export const MAGIC_CONTEXT_ID = new PublicKey('MagicContext1111111111111111111111111111111');

// Endpoints
export const BASE_LAYER_ENDPOINT = 'https://api.devnet.solana.com';
export const EPHEMERAL_ROLLUP_ENDPOINT = 'https://devnet.magicblock.app/';
export const EPHEMERAL_WS_ENDPOINT = 'wss://devnet.magicblock.app/';

// XFChess Program ID
export const PROGRAM_ID = new PublicKey('FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX');

/**
 * Dual connection manager for MagicBlock Ephemeral Rollups
 */
export class MagicBlockClient {
  // Base layer connection (Solana devnet/mainnet)
  baseConnection: Connection;
  
  // Ephemeral rollup connection
  erConnection: Connection;
  
  // Wallet adapter
  wallet: any;
  
  // Program instances
  baseProgram: Program;
  erProgram: Program;

  constructor(wallet: any) {
    this.wallet = wallet;
    
    // Base layer connection
    this.baseConnection = new Connection(BASE_LAYER_ENDPOINT, 'confirmed');
    
    // Ephemeral rollup connection with WebSocket
    this.erConnection = new Connection(EPHEMERAL_ROLLUP_ENDPOINT, {
      wsEndpoint: EPHEMERAL_WS_ENDPOINT,
      commitment: 'confirmed' as Commitment,
    });

    // Create providers
    const baseProvider = new AnchorProvider(
      this.baseConnection,
      wallet as any,
      { preflightCommitment: 'confirmed' }
    );

    const erProvider = new AnchorProvider(
      this.erConnection,
      wallet as any,
      { 
        preflightCommitment: 'confirmed',
        skipPreflight: true, // Critical: ER handles validation
      }
    );

    // Create program instances
    this.baseProgram = new Program(idl as Idl, baseProvider);
    this.erProgram = new Program(idl as Idl, erProvider);
  }

  /**
   * Check if an account is delegated to the Ephemeral Rollup
   */
  async isDelegated(pda: PublicKey): Promise<boolean> {
    const accountInfo = await this.baseConnection.getAccountInfo(pda);
    if (!accountInfo) return false;
    return accountInfo.owner.equals(DELEGATION_PROGRAM_ID);
  }

  /**
   * Get the appropriate program instance for an operation
   * - Use baseProgram for: initialization, delegation
   * - Use erProgram for: operations on delegated accounts, undelegation
   */
  getProgramForDelegated(pda: PublicKey): Promise<Program> {
    return this.isDelegated(pda).then(isDel => 
      isDel ? this.erProgram : this.baseProgram
    );
  }

  /**
   * Build a delegate transaction (sent to base layer)
   */
  async buildDelegateTx(
    gamePda: PublicKey,
    gameId: string
  ): Promise<{ instruction: any }> {
    const instruction = await this.baseProgram.methods
      .delegate(gameId)
      .accounts({
        payer: this.wallet.publicKey,
        gameAccount: gamePda,
      })
      .instruction();

    return { instruction };
  }

  /**
   * Execute on delegated account (sent to ephemeral rollup)
   */
  async executeOnDelegated(
    methodName: string,
    accounts: Record<string, PublicKey>,
    args?: any[]
  ): Promise<string> {
    let builder = (this.erProgram.methods as any)[methodName](...(args || []));
    
    // Add accounts
    builder = builder.accounts(accounts);
    
    // Send with skipPreflight for ER speed
    return await builder.rpc({ skipPreflight: true });
  }

  /**
   * Build undelegate transaction (sent to ephemeral rollup)
   */
  async buildUndelegateTx(
    gamePda: PublicKey
  ): Promise<{ instruction: any }> {
    const instruction = await this.erProgram.methods
      .undelegate()
      .accounts({
        payer: this.wallet.publicKey,
        gameAccount: gamePda,
        magicProgram: MAGIC_PROGRAM_ID,
        magicContext: MAGIC_CONTEXT_ID,
      })
      .instruction();

    return { instruction };
  }

  /**
   * Wait for commitment on base layer after undelegation
   */
  async waitForCommitment(
    txHash: string,
    timeoutMs: number = 60000
  ): Promise<string | null> {
    try {
      const commitTxHash = await Promise.race([
        GetCommitmentSignature(txHash, this.erConnection),
        new Promise<null>((_, reject) => 
          setTimeout(() => reject(new Error('Timeout')), timeoutMs)
        ),
      ]);
      return commitTxHash;
    } catch (err) {
      console.warn('Commitment check failed:', err);
      return null;
    }
  }
}

/**
 * Legacy compatibility: single connection program getter
 * Consider migrating to MagicBlockClient for full dual-connection support
 */
export function getAnchorProgram(connection: Connection, wallet: any) {
  const provider = new AnchorProvider(connection, wallet as any, {
    preflightCommitment: 'confirmed',
  });
  return new Program(idl as Idl, provider);
}

// Re-export for convenience
export { DELEGATION_PROGRAM_ID, GetCommitmentSignature };
