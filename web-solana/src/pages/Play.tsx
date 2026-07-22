import { motion } from 'framer-motion';
import { ArrowLeft, X, Rocket, Download, BookOpen } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';

const GITHUB_REPO = 'trilltino/XFChess';
const RELEASES_URL = `https://github.com/${GITHUB_REPO}/releases`;
const INSTRUCTIONS_URL = `https://github.com/${GITHUB_REPO}/blob/main/docs/INSTALL.md`;

// Asset filenames embed the version (e.g. XFChess-Setup-1.2.0.exe), so a
// direct link can't be hardcoded — resolve the latest release via the GitHub
// API and match by pattern, per docs/INSTALL.md's documented naming scheme.
const ASSET_PATTERNS: Record<'windows' | 'macos' | 'linux', RegExp> = {
  windows: /^XFChess-Setup-.*\.exe$/i,
  macos: /^XFChess-.*\.dmg$/i,
  linux: /^XFChess-linux-x86_64-.*\.tar\.gz$/i,
};

const downloadPlatform = async (platform: 'windows' | 'macos' | 'linux') => {
  // Open the tab synchronously on click so browsers don't treat the later
  // async navigation as a popup and block it.
  const tab = window.open('', '_blank');
  try {
    const res = await fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases/latest`);
    if (!res.ok) throw new Error(`GitHub API returned ${res.status}`);
    const release = await res.json();
    const asset = (release.assets || []).find((a: { name: string }) => ASSET_PATTERNS[platform].test(a.name));
    if (!asset) throw new Error(`No ${platform} asset found on latest release`);
    if (tab) tab.location.href = asset.browser_download_url;
    else window.location.href = asset.browser_download_url;
  } catch (err) {
    console.error('[XFChess Download] Falling back to releases page', err);
    if (tab) tab.location.href = RELEASES_URL;
    else window.open(RELEASES_URL, '_blank');
  }
};

const PlayPage = () => {
  const [showNotice, setShowNotice] = useState(true);
  const [launchStatus, setLaunchStatus] = useState<string | null>(null);
  const [launchError, setLaunchError] = useState<string | null>(null);

  const launchGame = () => {
    const walletPubkey = localStorage.getItem('xfchess_wallet') || localStorage.getItem('xfchess_wallet_pubkey') || '';
    const username = localStorage.getItem('xfchess_username') || '';
    const token = localStorage.getItem('xfchess_token') || '';
    const launchUrl = `xfchess://launch?pubkey=${encodeURIComponent(walletPubkey)}&username=${encodeURIComponent(username)}`;
    let pageHidden = false;
    let localApiLaunchSucceeded = false;

    setLaunchError(null);
    setLaunchStatus('Attempting to open XFChess desktop app...');

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        pageHidden = true;
        setLaunchStatus('XFChess launch request sent. If the app is installed, it should open now.');
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange, { once: true });

    window.setTimeout(() => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      if (!pageHidden && !localApiLaunchSucceeded) {
        setLaunchStatus(null);
        setLaunchError('Launch request was sent, but the browser did not switch to the desktop app. Check the browser prompt, confirm XFChess protocol is allowed, and make sure xfchess-tauri.exe is already running.');
        console.error('[XFChess Launch] Deep link did not hide the page.', {
          launchUrl,
          username,
          hasWallet: walletPubkey.length > 0,
          origin: window.location.origin,
        });
      }
    }, 2500);

    console.log('[XFChess Launch] Attempting deep link', {
      launchUrl,
      username,
      hasWallet: walletPubkey.length > 0,
      origin: window.location.origin,
    });

    // After deep link attempt, also try direct local API call to Tauri if running
    window.setTimeout(() => {
      if (!pageHidden) {
        console.log('[XFChess Launch] Attempting fallback local API call to Tauri');
        fetch('http://localhost:7454/api/game/launch', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            pubkey: walletPubkey,
            username: username,
            token: token || undefined,
          }),
        })
        .then(response => {
          if (response.ok) {
            localApiLaunchSucceeded = true;
            setLaunchStatus('Game launch triggered via local API.');
            setLaunchError(null);
            console.log('[XFChess Launch] Local API launch successful');
          } else {
            console.log('[XFChess Launch] Local API launch failed with status', response.status);
            setLaunchError(`Local API launch failed with status ${response.status}. Please ensure the XFChess app is running.`);
          }
        })
        .catch(err => {
          console.log('[XFChess Launch] Local API launch error', err);
          setLaunchError(`Local API launch error: ${err.message}. Please ensure the XFChess app is running on port 7454.`);
        });
      }
    }, 1500);

    try {
      window.location.href = launchUrl;
    } catch (error) {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      setLaunchStatus(null);
      setLaunchError('Browser blocked the launch request before it could be sent.');
      console.error('[XFChess Launch] Failed to assign deep link', error);
    }
  };

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section" style={{ position: 'relative' }}>
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Play XFChess</div>
        <h1 style={{ fontSize: '2.5rem', fontWeight: 900, marginBottom: '8px' }}>Ready to Move?</h1>
        <p style={{ color: 'var(--text-dim)', marginBottom: '32px' }}>Launch the desktop application to play wagering and tournament games.</p>

        {launchStatus && (
          <div style={{ marginBottom: '16px', padding: '12px 16px', borderRadius: '10px', background: 'rgba(255, 255, 255, 0.06)', border: '1px solid rgba(255, 255, 255, 0.15)', color: '#ffffff' }}>
            {launchStatus}
          </div>
        )}

        {launchError && (
          <div style={{ marginBottom: '16px', padding: '12px 16px', borderRadius: '10px', background: 'rgba(255, 80, 80, 0.12)', border: '1px solid rgba(255, 80, 80, 0.3)', color: '#ffd0d0' }}>
            {launchError}
          </div>
        )}

        <div style={{ display: 'flex', gap: '16px', marginTop: '32px', flexWrap: 'wrap' }}>
          <button
            onClick={launchGame}
            style={{
              padding: '18px 36px',
              background: 'rgba(255,255,255,0.12)',
              color: '#fff',
              borderRadius: '10px',
              fontWeight: 800,
              fontSize: '1.1rem',
              border: 'none',
              cursor: 'pointer',
              minWidth: '220px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '12px',
              boxShadow: '0 10px 30px rgba(255, 255, 255, 0.15)'
            }}
          >
            <Rocket size={20} />
            Launch Desktop App
          </button>

          <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
              <button
                onClick={() => downloadPlatform('windows')}
                style={{
                  padding: '16px 24px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#fff',
                  borderRadius: '10px',
                  fontWeight: 700,
                  fontSize: '0.9rem',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px'
                }}
              >
                <Download size={18} />
                Windows
              </button>
              <button
                onClick={() => downloadPlatform('macos')}
                style={{
                  padding: '16px 24px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#fff',
                  borderRadius: '10px',
                  fontWeight: 700,
                  fontSize: '0.9rem',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px'
                }}
              >
                <Download size={18} />
                macOS
              </button>
              <button
                onClick={() => downloadPlatform('linux')}
                style={{
                  padding: '16px 24px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#fff',
                  borderRadius: '10px',
                  fontWeight: 700,
                  fontSize: '0.9rem',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px'
                }}
              >
                <Download size={18} />
                Linux
              </button>
              <a
                href={INSTRUCTIONS_URL}
                target="_blank"
                rel="noopener noreferrer"
                style={{
                  padding: '16px 24px',
                  background: 'transparent',
                  color: 'var(--text-dim)',
                  borderRadius: '10px',
                  fontWeight: 700,
                  fontSize: '0.9rem',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  textDecoration: 'none'
                }}
              >
                <BookOpen size={18} />
                Instructions
              </a>
          </div>
        </div>

        <div style={{ marginTop: '48px', padding: '24px', background: 'rgba(255,255,255,0.02)', borderRadius: '12px', border: '1px solid rgba(255,255,255,0.05)' }}>
            <h3 style={{ margin: '0 0 12px 0', fontSize: '1.1rem' }}>First time playing?</h3>
            <p style={{ margin: 0, color: 'var(--text-dim)', fontSize: '0.9rem', lineHeight: 1.6 }}>
                Download the XFChess desktop client for your operating system above. Once installed, you can launch the game directly from this page or your applications folder.
            </p>
        </div>

        {showNotice && (
          <div style={{
            position: 'fixed',
            right: '20px',
            top: '50%',
            transform: 'translateY(-50%)',
            width: '280px',
            padding: '20px',
            background: 'rgba(0, 0, 0, 0.95)',
            border: '1px solid rgba(255, 255, 255, 0.15)',
            borderRadius: '12px',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4)',
            backdropFilter: 'blur(16px)',
            zIndex: 1000
          }}>
            <button
              onClick={() => setShowNotice(false)}
              style={{
                position: 'absolute',
                top: '8px',
                right: '8px',
                background: 'none',
                border: 'none',
                color: 'var(--text-dim)',
                cursor: 'pointer',
                padding: '4px'
              }}
            >
              <X size={16} />
            </button>
            <p style={{ margin: 0, fontSize: '0.85rem', color: 'var(--text-dim)', lineHeight: 1.6, marginBottom: '12px' }}>
              <strong style={{ color: 'var(--primary)' }}>Wagering Requirements:</strong> PvP wagering requires a Solana wallet and KYC verification.
            </p>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <Link to="/kyc" style={{ color: '#ffffff', fontWeight: 600, fontSize: '0.85rem' }}>Complete KYC</Link>
              <a href="https://solflare.com" target="_blank" rel="noopener noreferrer" style={{ color: '#ffffff', fontWeight: 600, fontSize: '0.85rem' }}>Create wallet on Solflare</a>
            </div>
          </div>
        )}
      </section>
    </motion.div>
  );
};

export default PlayPage;


