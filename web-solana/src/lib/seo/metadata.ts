/**
 * Per-route SEO metadata registry.
 *
 * Mirrors the `PageMetadata` pattern already proven out in this codebase's
 * sibling project (js_handyman/handyman/shared/src/metadata.rs) — a typed
 * struct with per-page-type factory constructors, kept as the single source
 * of truth so <SeoHead> never has route-specific logic embedded in it.
 *
 * `noindex` defaults to true and public routes must opt in explicitly
 * (deny-by-default), matching the same posture used in public/robots.txt
 * and in the reference project's `page_meta()` fallback.
 */

const SITE_URL = 'https://xfchess.com';
const DEFAULT_OG_IMAGE = `${SITE_URL}/og-image.png`;
const SITE_NAME = 'XFChess';

export interface PageMetadata {
  title: string;
  description: string;
  /** Path only (e.g. "/tournaments") — canonical/OG URLs are derived from it. */
  path: string;
  ogImage?: string;
  noindex?: boolean;
}

function page(path: string, title: string, description: string, ogImage?: string): PageMetadata {
  return { path, title: `${title} | ${SITE_NAME}`, description, ogImage, noindex: false };
}

function privatePage(path: string, title: string): PageMetadata {
  return { path, title: `${title} | ${SITE_NAME}`, description: '', noindex: true };
}

/** Static registry for the ~13 public marketing/content routes. */
export const PAGE_METADATA: Record<string, PageMetadata> = {
  home: page(
    '/home',
    'Competitive Chess Server',
    'Play competitive chess with real prizes. Join tournaments, climb the ranked ladder, and challenge players worldwide on XFChess.',
  ),
  features: page(
    '/features',
    'Features',
    'Ranked matchmaking, wagered PvP, Swiss-format tournaments, and on-chain game verification — see what XFChess offers.',
  ),
  play: page(
    '/play',
    'Play Now',
    'Download XFChess for Windows, macOS, or Linux and start playing ranked or wagered chess in minutes.',
  ),
  tournaments: page(
    '/tournaments',
    'Tournaments',
    'Browse live and upcoming XFChess tournaments — Swiss-format brackets with real prize pools.',
  ),
  computer: page(
    '/computer',
    'Play vs Computer',
    'Play chess against the XFChess engine in your browser — no download required.',
  ),
  players: page(
    '/players',
    'Players',
    'Look up XFChess player profiles, ratings, and match history.',
  ),
  legal: page(
    '/legal',
    'Legal',
    'Terms of service and legal information for XFChess players.',
  ),
  compliance: page(
    '/compliance',
    'Compliance',
    'XFChess compliance information for regulated jurisdictions.',
  ),
  antiCheat: page(
    '/anti-cheat',
    'Anti-Cheat',
    'How XFChess detects and prevents cheating in ranked and wagered games.',
  ),
  newsRelease: page(
    '/news/release',
    'Release Notes',
    'Latest XFChess release notes and platform updates.',
  ),
  launch: page(
    '/launch',
    'Launch',
    'XFChess is live — join the competitive chess platform built on Solana.',
  ),
  waitlist: page(
    '/waitlist',
    'Join the Waitlist',
    'Sign up for early access to new XFChess features and tournaments.',
  ),
};

/** Dynamic per-tournament metadata (used by /tournament/:id). */
export function forTournament(id: string | number, name?: string): PageMetadata {
  const title = name ? `${name} — Tournament` : `Tournament #${id}`;
  return page(
    `/tournament/${id}`,
    title,
    name
      ? `Standings, bracket, and details for the ${name} XFChess tournament.`
      : `Standings, bracket, and details for XFChess tournament #${id}.`,
  );
}

/** Non-indexable routes — wallet-gated, auth, or ephemeral in-game state. */
export const PRIVATE_PAGE_METADATA: Record<string, PageMetadata> = {
  verify: privatePage('/verify', 'Verify Profile'),
  wSetup: privatePage('/w_setup', 'Wallet Setup'),
  profile: privatePage('/profile', 'Profile'),
  createProfile: privatePage('/create-profile', 'Create Profile'),
  kyc: privatePage('/kyc', 'Identity Verification'),
  login: privatePage('/login', 'Sign In'),
  // /auth/login no longer has its own metadata entry — it's now a plain
  // redirect to /login (see App.tsx), not a rendered page.
  lichessCallback: privatePage('/auth/lichess/callback', 'Lichess Link'),
};

export function canonicalUrl(path: string): string {
  return `${SITE_URL}${path}`;
}

export function ogImageUrl(meta: PageMetadata): string {
  return meta.ogImage ?? DEFAULT_OG_IMAGE;
}

export { SITE_URL, SITE_NAME, DEFAULT_OG_IMAGE };
