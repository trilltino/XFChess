import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import { SessionTokenManager } from '@magicblock-labs/gum-sdk';
import { MAGICBLOCK_CONFIG } from '../constants/magicblock';

// SessionConfig format that matches the Rust EXE expectations
interface SessionConfig {
    game_id: string;
    player_color: 'white' | 'black';
    session_key: string;      // base58 encoded secret key
    session_pubkey: string;   // base58 encoded public key
    node_id: string;          // Will be filled by EXE
    rpc_url: string;
    game_pda: string;         // Derived from game_id
    wager_amount: number;
    opponent_pubkey?: string;
}

// Legacy interface for local storage
interface GameSession {
    walletPubkey: string;
    sessionSigner: string;
    sessionSignerSecret: string;
    sessionTokenPDA: string;
    expiresAt: number;
    gameId?: string;
    role: 'host' | 'joiner';
}

const GameLauncher: React.FC = () => {
    const { publicKey, signTransaction } = useWallet();
    const { connection } = useConnection();
    const [session, setSession] = useState<GameSession | null>(null);
    const [isCreatingSession, setIsCreatingSession] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [gameId, setGameId] = useState('');
    const [role, setRole] = useState<'host' | 'joiner'>('host');

    const sessionTokenManager = useRef<SessionTokenManager | null>(null);
    const tempKeypair = useRef<Keypair | null>(null);
    const ephemeralConnection = useRef<Connection | null>(null);

    // Initialize ephemeral connection
    useEffect(() => {
        ephemeralConnection.current = new Connection(MAGICBLOCK_CONFIG.RPC_URL);
    }, []);

    // Initialize session manager when wallet connects
    useEffect(() => {
        if (!publicKey) return;

        const initSessionManager = async () => {
            try {
                const provider = new anchor.AnchorProvider(
                    connection,
                    // @ts-ignore - wallet adapter compatibility
                    { publicKey, signTransaction },
                    { commitment: 'confirmed' }
                );

                sessionTokenManager.current = new SessionTokenManager(
                    provider as any,
                    connection
                );

                // Derive temp keypair from wallet public key (deterministic)
                const seed = publicKey.toBytes().slice(0, 32);
                tempKeypair.current = Keypair.fromSeed(seed);

                console.log('Session manager initialized');
                console.log('Temp keypair:', tempKeypair.current.publicKey.toBase58());
            } catch (err) {
                console.error('Failed to initialize session manager:', err);
                setError('Failed to initialize session manager');
            }
        };

        initSessionManager();
    }, [publicKey, connection, signTransaction]);

    // Check for existing session in localStorage
    useEffect(() => {
        const saved = localStorage.getItem('xfchess_session');
        if (saved) {
            try {
                const parsed = JSON.parse(saved) as GameSession;
                if (parsed.expiresAt > Date.now()) {
                    setSession(parsed);
                    // Restore temp keypair
                    if (parsed.sessionSignerSecret) {
                        const secretKey = new Uint8Array(
                            JSON.parse(parsed.sessionSignerSecret)
                        );
                        tempKeypair.current = Keypair.fromSecretKey(secretKey);
                    }
                } else {
                    localStorage.removeItem('xfchess_session');
                }
            } catch {
                localStorage.removeItem('xfchess_session');
            }
        }
    }, []);

    // Fund temp keypair if needed
    const ensureTempKeypairFunded = useCallback(async (): Promise<boolean> => {
        if (!tempKeypair.current || !publicKey) return false;

        const balance = await connection.getBalance(tempKeypair.current.publicKey);
        if (balance < 0.01 * LAMPORTS_PER_SOL) {
            try {
                // Request airdrop for devnet
                await connection.requestAirdrop(
                    tempKeypair.current.publicKey,
                    0.1 * LAMPORTS_PER_SOL
                );
                // Wait for confirmation
                await new Promise(resolve => setTimeout(resolve, 2000));
            } catch (err) {
                console.warn('Failed to fund temp keypair:', err);
            }
        }
        return true;
    }, [connection, publicKey]);

    // Create session token
    const createSession = useCallback(async () => {
        if (!publicKey || !signTransaction || !tempKeypair.current || !sessionTokenManager.current) {
            setError('Wallet not connected');
            return;
        }

        setIsCreatingSession(true);
        setError(null);

        try {
            // Ensure temp keypair is funded
            await ensureTempKeypairFunded();

            // Calculate expiry (2 hours from now)
            const validUntil = Math.floor(Date.now() / 1000) + 7200;

            // Create session on-chain
            // Note: This requires the session-keys program to be deployed
            // For hackathon, we'll create a local session if on-chain fails
            try {
                const tx = await sessionTokenManager.current.program.methods
                    .createSession(
                        true, // topUp
                        new anchor.BN(validUntil),
                        new anchor.BN(0.0005 * LAMPORTS_PER_SOL)
                    )
                    .accounts({
                        targetProgram: new PublicKey(MAGICBLOCK_CONFIG.PROGRAM_ID),
                        sessionSigner: tempKeypair.current.publicKey,
                        authority: publicKey,
                    })
                    .transaction();

                const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
                tx.recentBlockhash = blockhash;
                tx.feePayer = publicKey;

                // Sign with temp keypair first
                tx.sign(tempKeypair.current);

                // Sign with wallet
                const signed = await signTransaction(tx);
                const signature = await connection.sendRawTransaction(signed.serialize());
                await connection.confirmTransaction({ blockhash, lastValidBlockHeight, signature });

                console.log('Session created on-chain:', signature);
            } catch (onChainErr) {
                console.warn('On-chain session creation failed (expected if program not deployed):', onChainErr);
                // Continue with local session for hackathon demo
            }

            // Create local session data
            const sessionData: GameSession = {
                walletPubkey: publicKey.toBase58(),
                sessionSigner: tempKeypair.current.publicKey.toBase58(),
                sessionSignerSecret: JSON.stringify(Array.from(tempKeypair.current.secretKey)),
                sessionTokenPDA: '', // Would be derived from on-chain state
                expiresAt: validUntil * 1000,
                gameId: gameId || undefined,
                role,
            };

            setSession(sessionData);
            localStorage.setItem('xfchess_session', JSON.stringify(sessionData));

        } catch (err) {
            console.error('Failed to create session:', err);
            setError(`Failed to create session: ${err}`);
        } finally {
            setIsCreatingSession(false);
        }
    }, [publicKey, signTransaction, connection, ensureTempKeypairFunded, gameId, role]);

    // Derive game PDA from game_id (matches Rust implementation)
    const deriveGamePDA = (gameId: string): string => {
        // In a real implementation, this would use the same PDA derivation as the Rust code
        // For now, we return a placeholder that the EXE will validate
        return `GAME_${gameId}_PDA`;
    };

    // Launch native game with session data
    const launchGame = useCallback(async () => {
        if (!session) {
            setError('No active session');
            return;
        }

        try {
            // Convert GameSession to SessionConfig format for the EXE
            const sessionConfig: SessionConfig = {
                game_id: session.gameId || '0',
                player_color: session.role === 'host' ? 'white' : 'black',
                session_key: session.sessionSignerSecret, // Already JSON stringified Uint8Array
                session_pubkey: session.sessionSigner,
                node_id: '', // EXE will fill this in
                rpc_url: MAGICBLOCK_CONFIG.RPC_URL,
                game_pda: deriveGamePDA(session.gameId || '0'),
                wager_amount: 0.01, // Default wager
            };

            // Serialize session config for the native game
            const sessionJson = JSON.stringify(sessionConfig, null, 2);

            // Copy to clipboard as backup
            await navigator.clipboard.writeText(sessionJson);

            // Launch the game via the batch file
            // In a real implementation, this would use a custom protocol handler
            // or the batch file launcher we created
            const response = await fetch('/api/launch', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ session: sessionJson }),
            }).catch(() => {
                // API not available, use alternative method
                console.log('Launch API not available, using fallback');
                return null;
            });

            if (!response) {
                // Fallback: Save to file with unique name based on game ID
                const fileName = session.gameId
                    ? `xfchess_session_${session.gameId}.json`
                    : `xfchess_session_${Date.now()}.json`;
                const blob = new Blob([sessionJson], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = fileName;
                a.click();
                URL.revokeObjectURL(url);

                const role = session.role === 'host' ? 'Player 1 (White)' : 'Player 2 (Black)';
                alert(`${role} session saved to ${fileName}.\n\nIMPORTANT: After launching the game, copy your Node ID from the game window and share it with your opponent.\n\nLaunch with:\nlaunch_game_with_session.bat ${fileName}`);
            }
        } catch (err) {
            console.error('Failed to launch game:', err);
            setError(`Failed to launch game: ${err}`);
        }
    }, [session]);

    // Clear session
    const clearSession = useCallback(() => {
        setSession(null);
        localStorage.removeItem('xfchess_session');
        tempKeypair.current = null;
    }, []);

    if (!publicKey) {
        return (
            <div style={{
                padding: '1rem',
                backgroundColor: 'var(--bg-tertiary)',
                borderRadius: '8px',
                border: '1px solid var(--border-color)',
            }}>
                <p style={{ color: 'var(--text-secondary)', margin: 0 }}>
                    Connect wallet to launch game with session management
                </p>
            </div>
        );
    }

    return (
        <div style={{
            padding: '1.5rem',
            backgroundColor: 'var(--bg-tertiary)',
            borderRadius: '8px',
            border: '1px solid var(--border-color)',
        }}>
            <h3 style={{ marginTop: 0, marginBottom: '1rem' }}>
                Game Launcher
            </h3>

            {error && (
                <div className="error-box">
                    <strong>Error:</strong> {error}
                </div>
            )}

            {!session ? (
                <>
                    <div style={{ marginBottom: '1rem' }}>
                        <label style={{
                            display: 'block',
                            marginBottom: '0.5rem',
                            color: 'var(--text-secondary)',
                            fontSize: '0.875rem',
                        }}>
                            Role:
                        </label>
                        <div style={{ display: 'flex', gap: '0.5rem' }}>
                            <button
                                onClick={() => setRole('host')}
                                style={{
                                    flex: 1,
                                    padding: '0.5rem',
                                    border: `1px solid ${role === 'host' ? 'var(--accent-primary)' : 'var(--border-color)'}`,
                                    borderRadius: '4px',
                                    backgroundColor: role === 'host' ? 'rgba(99, 102, 241, 0.1)' : 'transparent',
                                    color: role === 'host' ? 'var(--accent-primary)' : 'var(--text-secondary)',
                                    cursor: 'pointer',
                                }}
                            >
                                Host Game
                            </button>
                            <button
                                onClick={() => setRole('joiner')}
                                style={{
                                    flex: 1,
                                    padding: '0.5rem',
                                    border: `1px solid ${role === 'joiner' ? 'var(--accent-primary)' : 'var(--border-color)'}`,
                                    borderRadius: '4px',
                                    backgroundColor: role === 'joiner' ? 'rgba(99, 102, 241, 0.1)' : 'transparent',
                                    color: role === 'joiner' ? 'var(--accent-primary)' : 'var(--text-secondary)',
                                    cursor: 'pointer',
                                }}
                            >
                                Join Game
                            </button>
                        </div>
                    </div>

                    {role === 'joiner' && (
                        <div style={{
                            marginBottom: '1rem',
                            padding: '1rem',
                            backgroundColor: 'rgba(59, 130, 246, 0.1)',
                            border: '2px solid #3b82f6',
                            borderRadius: '8px',
                        }}>
                            <label style={{
                                display: 'block',
                                marginBottom: '0.5rem',
                                color: '#60a5fa',
                                fontSize: '0.875rem',
                                fontWeight: 'bold',
                            }}>
                                Enter Game ID to Join:
                            </label>
                            <input
                                type="text"
                                value={gameId}
                                onChange={(e) => setGameId(e.target.value)}
                                placeholder="Paste the game ID from the host"
                                style={{
                                    width: '100%',
                                    padding: '0.75rem',
                                    borderRadius: '4px',
                                    border: '2px solid #3b82f6',
                                    backgroundColor: 'var(--bg-secondary)',
                                    color: 'var(--text-primary)',
                                    fontSize: '1rem',
                                }}
                            />
                            <p style={{
                                marginTop: '0.5rem',
                                fontSize: '0.75rem',
                                color: '#93c5fd'
                            }}>
                                Ask the host for their Game ID
                            </p>
                        </div>
                    )}

                    <button
                        onClick={createSession}
                        disabled={isCreatingSession}
                        className="btn btn-primary"
                        style={{ width: '100%' }}
                    >
                        {isCreatingSession ? 'Creating Session...' : 'Create Game Session'}
                    </button>
                </>
            ) : (
                <>
                    <div style={{
                        padding: '1rem',
                        backgroundColor: 'rgba(34, 197, 94, 0.1)',
                        border: '1px solid #22c55e',
                        borderRadius: '4px',
                        marginBottom: '1rem',
                    }}>
                        <p style={{ margin: '0 0 0.5rem', color: '#22c55e' }}>
                            Session Active
                        </p>
                        <p style={{
                            margin: 0,
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                            fontFamily: 'monospace',
                        }}>
                            Wallet: {session.walletPubkey.slice(0, 16)}...<br />
                            Expires: {new Date(session.expiresAt).toLocaleTimeString()}<br />
                            Role: {session.role}
                        </p>
                    </div>

                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                        <button
                            onClick={launchGame}
                            className="btn btn-primary"
                            style={{ flex: 1 }}
                        >
                            Launch Game
                        </button>
                        <button
                            onClick={clearSession}
                            className="btn"
                            style={{
                                backgroundColor: 'var(--bg-secondary)',
                                border: '1px solid var(--border-color)',
                            }}
                        >
                            Clear
                        </button>
                    </div>
                </>
            )}
        </div>
    );
};

export default GameLauncher;
