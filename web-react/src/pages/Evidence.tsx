import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
    Shield,
    CheckCircle,
    XCircle,
    ExternalLink,
    Database,
    Zap,
    ChevronDown,
    FileText,
    Clock
} from 'lucide-react';
import './Evidence.css';

// Test evidence data
const solanaEvidence = [
    {
        id: 'delegation-1',
        name: 'Game Delegation',
        status: 'success',
        signature: '5QPvDzSUcZifD12vtAPkuACufLFTrr8Z3yVhPQb2K5PR9AxyYMTYmjrM2Xwo2BtwCUdQNARFSQpjNoBuiQvtZQdD',
        gamePda: 'F4QtXmUMf2ckyNzYqNmTE3s73uhhNtK6PkHRDq2zWXJy',
        timestamp: '2026-02-27T13:36:04.154Z',
        description: 'Game PDA delegation transaction sent to Solana devnet'
    },
    {
        id: 'wager-init',
        name: 'Wager Initialization',
        status: 'success',
        signature: 'nsQ4UxwN3PQMQwffM7q8CRAytEc5YYW5Jn2dQyBx7t8MJNhajjtxo9e5pRv9AcEmnSQGBtSc15LGePGYpFykigt',
        gamePda: '8uHYCw4AjYmdhxkfeHFZNMVNNNAvLbrnNkSVbn4e3kDx',
        slot: 445032680,
        timestamp: '2026-02-27T13:36:07Z',
        description: 'Player 1 initialized game with 0.01 SOL wager'
    },
    {
        id: 'wager-join',
        name: 'Player 2 Join',
        status: 'success',
        signature: 'BecCw1XPTb45SstScQ6Cv2QzsKRfRCfp8qDPgGf5yXp61ya1gGP9hd1u2y1Mzfv47xRTEmoJGd8RxTZnky8mpbk',
        gamePda: '8uHYCw4AjYmdhxkfeHFZNMVNNNAvLbrnNkSVbn4e3kDx',
        slot: 445032683,
        timestamp: '2026-02-27T13:36:10Z',
        description: 'Player 2 joined with matching 0.01 SOL wager'
    }
];

const erEvidence = [
    {
        id: 'er-delegation',
        name: 'ER Delegation Attempt',
        status: 'pending',
        description: 'Real ER delegation requires program integration',
        note: 'Tests attempted ER routing but fell back to Solana'
    },
    {
        id: 'er-gameplay',
        name: 'ER Gameplay Attempt',
        status: 'pending',
        description: 'Sub-second moves through ER',
        note: 'Requires game to be delegated to ER validator first'
    },
    {
        id: 'er-undelegation',
        name: 'ER Undelegation Attempt',
        status: 'pending',
        description: 'Commit final state to Solana',
        note: 'Requires active ER delegation'
    }
];

const TestCard = ({ test, type }: { test: any; type: 'solana' | 'er' }) => {
    const [expanded, setExpanded] = useState(false);

    const getExplorerUrl = (signature: string) => {
        return `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
    };

    const getAddressUrl = (address: string) => {
        return `https://explorer.solana.com/address/${address}?cluster=devnet`;
    };

    return (
        <motion.div
            className={`test-card ${test.status}`}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3 }}
        >
            <div className="test-header" onClick={() => setExpanded(!expanded)}>
                <div className="test-status-icon">
                    {test.status === 'success' ? (
                        <CheckCircle className="icon-success" />
                    ) : test.status === 'error' ? (
                        <XCircle className="icon-error" />
                    ) : (
                        <Clock className="icon-pending" />
                    )}
                </div>
                <div className="test-info">
                    <h3>{test.name}</h3>
                    <p className="test-description">{test.description}</p>
                </div>
                <ChevronDown className={`expand-icon ${expanded ? 'expanded' : ''}`} />
            </div>

            <AnimatePresence>
                {expanded && (
                    <motion.div
                        className="test-details"
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.2 }}
                    >
                        {test.signature && (
                            <div className="detail-row">
                                <span className="detail-label">Signature:</span>
                                <a
                                    href={getExplorerUrl(test.signature)}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="detail-value link"
                                >
                                    {test.signature.slice(0, 20)}...{test.signature.slice(-8)}
                                    <ExternalLink className="link-icon" />
                                </a>
                            </div>
                        )}

                        {test.gamePda && (
                            <div className="detail-row">
                                <span className="detail-label">Game PDA:</span>
                                <a
                                    href={getAddressUrl(test.gamePda)}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="detail-value link"
                                >
                                    {test.gamePda.slice(0, 12)}...{test.gamePda.slice(-8)}
                                    <ExternalLink className="link-icon" />
                                </a>
                            </div>
                        )}

                        {test.slot && (
                            <div className="detail-row">
                                <span className="detail-label">Slot:</span>
                                <span className="detail-value">{test.slot.toLocaleString()}</span>
                            </div>
                        )}

                        {test.timestamp && (
                            <div className="detail-row">
                                <span className="detail-label">Timestamp:</span>
                                <span className="detail-value">{new Date(test.timestamp).toLocaleString()}</span>
                            </div>
                        )}

                        {test.from && (
                            <div className="detail-row">
                                <span className="detail-label">From:</span>
                                <a
                                    href={getAddressUrl(test.from)}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="detail-value link"
                                >
                                    {test.from.slice(0, 8)}...{test.from.slice(-8)}
                                    <ExternalLink className="link-icon" />
                                </a>
                            </div>
                        )}

                        {test.to && (
                            <div className="detail-row">
                                <span className="detail-label">To:</span>
                                <a
                                    href={getAddressUrl(test.to)}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="detail-value link"
                                >
                                    {test.to.slice(0, 8)}...{test.to.slice(-8)}
                                    <ExternalLink className="link-icon" />
                                </a>
                            </div>
                        )}

                        {test.amount && (
                            <div className="detail-row">
                                <span className="detail-label">Amount:</span>
                                <span className="detail-value highlight">{test.amount}</span>
                            </div>
                        )}

                        {test.note && (
                            <div className="detail-note">
                                <FileText className="note-icon" />
                                <span>{test.note}</span>
                            </div>
                        )}
                    </motion.div>
                )}
            </AnimatePresence>
        </motion.div>
    );
};

const Evidence = () => {
    const [activeTab, setActiveTab] = useState<'solana' | 'er'>('solana');

    return (
        <motion.div
            className="evidence-page"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.3 }}
        >
            <div className="evidence-container">
                <motion.div
                    className="evidence-header"
                    initial={{ opacity: 0, y: -20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.4 }}
                >
                    <div className="header-icon">
                        <Shield className="shield-icon" />
                    </div>
                    <h1>Test Evidence</h1>
                    <p className="subtitle">
                        On-chain verification of XFChess testing infrastructure
                    </p>
                </motion.div>

                <div className="network-tabs">
                    <button
                        className={`tab ${activeTab === 'solana' ? 'active' : ''}`}
                        onClick={() => setActiveTab('solana')}
                    >
                        <Database className="tab-icon" />
                        <span>Solana</span>
                        <span className="badge success">{solanaEvidence.length} Verified</span>
                    </button>
                    <button
                        className={`tab ${activeTab === 'er' ? 'active' : ''}`}
                        onClick={() => setActiveTab('er')}
                    >
                        <Zap className="tab-icon" />
                        <span>MagicBlock ER</span>
                        <span className="badge pending">Pending</span>
                    </button>
                </div>

                <div className="evidence-content">
                    {activeTab === 'solana' ? (
                        <motion.div
                            initial={{ opacity: 0, x: -20 }}
                            animate={{ opacity: 1, x: 0 }}
                            transition={{ duration: 0.3 }}
                        >
                            <div className="section-header">
                                <h2>Solana Devnet Transactions</h2>
                                <p className="section-description">
                                    Successfully tested and verified on Solana devnet.
                                    All transactions are real and can be verified on the explorer.
                                </p>
                            </div>

                            <div className="stats-bar">
                                <div className="stat">
                                    <span className="stat-value">{solanaEvidence.length}</span>
                                    <span className="stat-label">Transactions</span>
                                </div>
                                <div className="stat">
                                    <span className="stat-value success">3</span>
                                    <span className="stat-label">Successful</span>
                                </div>
                                <div className="stat">
                                    <span className="stat-value">0.02</span>
                                    <span className="stat-label">SOL Tested</span>
                                </div>
                            </div>

                            <div className="test-list">
                                {solanaEvidence.map((test) => (
                                    <TestCard key={test.id} test={test} type="solana" />
                                ))}
                            </div>

                            <div className="explorer-link">
                                <a
                                    href="https://explorer.solana.com/?cluster=devnet"
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="btn-primary"
                                >
                                    <ExternalLink className="btn-icon" />
                                    View Solana Devnet Explorer
                                </a>
                            </div>
                        </motion.div>
                    ) : (
                        <motion.div
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            transition={{ duration: 0.3 }}
                        >
                            <div className="section-header">
                                <h2>MagicBlock Ephemeral Rollups</h2>
                                <p className="section-description">
                                    ER-specific features require actual program integration.
                                    These tests demonstrate the infrastructure is ready for ER delegation.
                                </p>
                            </div>

                            <div className="info-box">
                                <Zap className="info-icon" />
                                <div className="info-content">
                                    <h3>What is MagicBlock ER?</h3>
                                    <p>
                                        MagicBlock Ephemeral Rollups enable sub-second transaction processing
                                        by delegating game state to an ER validator during gameplay, then
                                        committing the final state back to Solana.
                                    </p>
                                </div>
                            </div>

                            <div className="test-list">
                                {erEvidence.map((test) => (
                                    <TestCard key={test.id} test={test} type="er" />
                                ))}
                            </div>

                            <div className="implementation-note">
                                <h4>Implementation Status</h4>
                                <p>
                                    The Rust code in <code>src/multiplayer/magicblock_resolver.rs</code> contains
                                    the ER integration. To test real ER functionality, the game must be built with
                                    the solana feature enabled and use the actual program instructions.
                                </p>
                            </div>

                            <div className="explorer-link">
                                <a
                                    href="https://docs.magicblock.gg/"
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="btn-secondary"
                                >
                                    <ExternalLink className="btn-icon" />
                                    MagicBlock Documentation
                                </a>
                            </div>
                        </motion.div>
                    )}
                </div>
            </div>
        </motion.div>
    );
};

export default Evidence;
