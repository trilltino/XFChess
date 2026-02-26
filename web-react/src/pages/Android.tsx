import { motion } from 'framer-motion';
import { ArrowLeft, Download } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './Android.css';

const Android = () => {
    return (
        <motion.div
            className="android-container"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.5 }}
        >
            <Link to="/" className="back-btn" style={{ position: 'absolute', top: '2rem', left: '2rem', display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#e63946', textDecoration: 'none', fontWeight: 'bold' }}>
                <ArrowLeft size={18} /> Back
            </Link>

            <div className="android-header">
                <div className="section-label" style={{ color: '#e63946', fontSize: '0.75rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', marginBottom: '0.75rem' }}>Mobile</div>
                <h1>XFChess for Android</h1>
                <p>Full on-chain chess in your pocket — Braid P2P, Stockfish AI, and Solana settlement on mobile.</p>
            </div>

            {/* STATUS BANNER */}
            <div className="android-status-banner">
                <div className="android-status-dot" />
                <span>In Development — Targeting Android 12+ (API 31)</span>
            </div>

            {/* OVERVIEW */}
            <section className="android-section">
                <h2 className="android-section-title">Overview</h2>
                <p className="android-section-desc">
                    XFChess Android brings the full desktop experience to mobile without compromise. The same Bevy game engine that powers the desktop client runs natively on Android via <strong>Bevy's Android target</strong>, giving you hardware-accelerated 3D rendering, the complete Stockfish AI engine, and the full Braid + Iroh P2P networking stack — all running locally on your device. There is no separate mobile codebase: the Android build shares the same Rust game logic, smart contract integrations, and networking layer as the desktop client, ensuring feature parity from day one.
                </p>
                <p className="android-section-desc">
                    Solana wallet integration on Android is handled via <strong>Mobile Wallet Adapter (MWA)</strong> — the standard protocol for connecting dApps to Android wallets like Phantom and Solflare. When a match concludes, the final game state is signed and submitted to Solana directly from your phone, with the same on-chain ELO updates, wager settlements, and move log commitments as the desktop version.
                </p>
            </section>

            <div className="android-divider" />


            <div className="android-divider" />

            {/* TECHNICAL DETAILS */}
            <section className="android-section">
                <h2 className="android-section-title">Technical Architecture</h2>
                <p className="android-section-desc">
                    The Android build uses Bevy's <code>NativeActivity</code> integration via the <code>android-activity</code> crate, which provides a Rust-native entry point without requiring a Java/Kotlin wrapper. The Gradle build system invokes <code>cargo ndk</code> to cross-compile the Rust workspace for <code>aarch64-linux-android</code> and <code>armv7-linux-androideabi</code> targets, producing a single APK that supports both 64-bit and 32-bit ARM devices.
                </p>
                <p className="android-section-desc">
                    Stockfish is compiled separately as a standalone ARM binary bundled in the APK's <code>assets/</code> directory and extracted to the app's private storage on first launch. The UCI communication channel uses Android's <code>LocalSocket</code> API rather than standard pipes, avoiding the file descriptor limitations of the Android sandbox. The Iroh endpoint binds to a local UDP port and uses Android's <code>ConnectivityManager</code> to detect network changes and trigger connection migration — ensuring P2P games survive a switch from Wi-Fi to mobile data.
                </p>

                <CodeViewer
                    title="src/platform/android.rs"
                    language="Rust (Bevy Android)"
                    code={`#[cfg(target_os = "android")]
pub fn android_main(app: AndroidApp) {
    // Initialise Bevy with Android-specific plugins
    App::new()
        .insert_resource(AndroidApp::from(app))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resizable: false,
                mode: WindowMode::BorderlessFullscreen,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(XFChessGamePlugin)
        .add_plugins(BraidNetworkPlugin)
        .add_plugins(StockfishPlugin::with_depth_limit(12)) // reduced for mobile
        .add_plugins(SolanaPlugin::with_mwa()) // Mobile Wallet Adapter
        .run();
}`}
                />
            </section>

            <div className="android-divider" />

            {/* DOWNLOAD CTA */}
            <section className="android-section android-cta-section">
                <h2>Coming Soon</h2>
                <p className="android-section-desc">The Android build is currently in active development. Follow the GitHub repository for release announcements and pre-release APKs.</p>
                <a
                    href="https://github.com/trilltino/XFChess"
                    className="android-cta-btn"
                    target="_blank"
                    rel="noreferrer"
                >
                    <Download size={18} />
                    Watch on GitHub
                </a>
            </section>
        </motion.div>
    );
};

export default Android;
