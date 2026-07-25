import { motion } from 'framer-motion';
import { ArrowLeft, BookOpen } from 'lucide-react';
import { Link } from 'react-router-dom';
import { WindowsIcon, MacIcon, LinuxIcon } from '../components/PlatformIcons';
import { SeoHead } from '../components/SeoHead';
import { VideoGameSchema } from '../components/StructuredData';
import { PAGE_METADATA } from '../lib/seo/metadata';

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
  // Redirect the current page straight to the asset. GitHub serves release
  // assets with Content-Disposition: attachment, so this triggers a direct
  // file download in place — no new tab, no intermediate page.
  try {
    const res = await fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases/latest`);
    if (!res.ok) throw new Error(`GitHub API returned ${res.status}`);
    const release = await res.json();
    const asset = (release.assets || []).find((a: { name: string }) => ASSET_PATTERNS[platform].test(a.name));
    if (!asset) throw new Error(`No ${platform} asset found on latest release`);
    window.location.href = asset.browser_download_url;
  } catch (err) {
    console.error('[XFChess Download] Falling back to releases page', err);
    window.location.href = RELEASES_URL;
  }
};

const PlayPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <SeoHead meta={PAGE_METADATA.play} />
      <VideoGameSchema />
      <section className="section" style={{ position: 'relative' }}>
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '24px', marginTop: '32px', alignItems: 'flex-start' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', alignItems: 'flex-start' }}>
              <button
                onClick={() => downloadPlatform('windows')}
                style={{
                  padding: '20px 28px',
                  background: 'transparent',
                  border: 'none',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '20px',
                  textAlign: 'left'
                }}
              >
                <WindowsIcon size={64} />
                <div>
                  <div style={{ color: '#fff', fontWeight: 700, fontSize: '1.15rem' }}>Windows</div>
                  <div style={{ color: 'var(--text-dim)', fontWeight: 400, fontSize: '0.95rem', marginTop: '4px' }}>
                    For Windows
                  </div>
                </div>
              </button>
              <button
                onClick={() => downloadPlatform('macos')}
                style={{
                  padding: '20px 28px',
                  background: 'transparent',
                  border: 'none',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '20px',
                  textAlign: 'left'
                }}
              >
                <MacIcon size={64} />
                <div>
                  <div style={{ color: '#fff', fontWeight: 700, fontSize: '1.15rem' }}>macOS</div>
                  <div style={{ color: 'var(--text-dim)', fontWeight: 400, fontSize: '0.95rem', marginTop: '4px' }}>
                    For macOS
                  </div>
                </div>
              </button>
              <button
                onClick={() => downloadPlatform('linux')}
                style={{
                  padding: '20px 28px',
                  background: 'transparent',
                  color: '#fff',
                  border: 'none',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '20px',
                  textAlign: 'left'
                }}
              >
                <LinuxIcon size={64} />
                <div>
                  <div style={{ color: '#fff', fontWeight: 700, fontSize: '1.15rem' }}>Linux</div>
                  <div style={{ color: 'var(--text-dim)', fontWeight: 400, fontSize: '0.95rem', marginTop: '4px' }}>
                    For Linux
                  </div>
                </div>
              </button>
              <a
                href={INSTRUCTIONS_URL}
                target="_blank"
                rel="noopener noreferrer"
                style={{
                  padding: '20px 28px',
                  background: 'transparent',
                  color: 'var(--text-dim)',
                  fontWeight: 700,
                  fontSize: '1.15rem',
                  border: 'none',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '20px',
                  textDecoration: 'none'
                }}
              >
                <BookOpen size={48} />
                Instructions
              </a>
          </div>
        </div>

      </section>
    </motion.div>
  );
};

export default PlayPage;


