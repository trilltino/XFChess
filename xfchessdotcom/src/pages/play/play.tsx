import { motion } from 'framer-motion';
import { WindowsIcon, MacIcon, LinuxIcon, ChromeOSIcon } from '../../components/PlatformIcons';
import { SeoHead } from '../../components/SeoHead';
import { VideoGameSchema } from '../../components/StructuredData';
import { PAGE_METADATA } from '../../lib/seo/metadata';

const GITHUB_REPO = 'trilltino/XFChess';
const RELEASES_URL = `https://github.com/${GITHUB_REPO}/releases`;
const INSTRUCTIONS_URL = `https://github.com/${GITHUB_REPO}/blob/main/docs/INSTALL.md`;

// Asset filenames embed the version (e.g. XFChess-Setup-1.2.0.exe), so a
// direct link can't be hardcoded — resolve the latest release via the GitHub
// API and match by pattern, per docs/INSTALL.md's documented naming scheme.
const ASSET_PATTERNS: Record<'windows' | 'macos' | 'linux' | 'chromeos', RegExp> = {
  windows: /^XFChess-Setup-.*\.exe$/i,
  macos: /^XFChess-.*\.dmg$/i,
  linux: /^XFChess-linux-x86_64-.*\.tar\.gz$/i,
  chromeos: /^XFChess-chromeos-.*\.zip$/i,
};

const downloadPlatform = async (platform: 'windows' | 'macos' | 'linux' | 'chromeos') => {
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

const PLATFORMS: { id: 'windows' | 'macos' | 'linux' | 'chromeos'; label: string; Icon: typeof WindowsIcon }[] = [
  { id: 'windows', label: 'Windows', Icon: WindowsIcon },
  { id: 'macos', label: 'macOS', Icon: MacIcon },
  { id: 'linux', label: 'Linux', Icon: LinuxIcon },
  { id: 'chromeos', label: 'Chrome OS', Icon: ChromeOSIcon },
];

const PlayPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
      <SeoHead meta={PAGE_METADATA.play} />
      <VideoGameSchema />
      <main className="mp">

        <div className="mp-rows mp-downloads">
          {PLATFORMS.map(({ id, label, Icon }) => (
            <div key={id} className="mp-dl">
              <span className="mp-dl-name">
                <Icon size={40} />
                <button
                  type="button"
                  className="mp-dl-trigger"
                  onClick={() => downloadPlatform(id)}
                  aria-label={`Download XFChess for ${label}`}
                >
                  {label}
                </button>
              </span>
              <span className="mp-dl-cta"> </span>
            </div>
          ))}
        </div>
        <p className="mp-note">
          <a href={INSTRUCTIONS_URL} target="_blank" rel="noopener noreferrer">Instructions</a>
          {' · '}
          <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">All releases</a>
        </p>
      </main>
    </motion.div>
  );
};
export default PlayPage;