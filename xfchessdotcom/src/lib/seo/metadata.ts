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
  return { path, title: `${SITE_NAME} | ${title}`, description, ogImage, noindex: false };
}

/** Static registry for the four public routes. */
export const PAGE_METADATA: Record<string, PageMetadata> = {
  home: page(
    '/home',
    'Competitive Chess Server',
    'Play competitive chess with real prizes. Join tournaments, climb the ranked ladder, and challenge players worldwide on XFChess.',
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
  features: page(
    '/features',
    'Features',
    'Ranked matchmaking, wagered PvP, Swiss-format tournaments, and on-chain game verification — see what XFChess offers.',
  ),
};

// PRIVATE_PAGE_METADATA, the privatePage() constructor, and forTournament()
// went with the pages that used them (verify, w_setup, profile,
// create-profile, kyc, login, the Lichess OAuth return leg, and
// /tournament/:id). Every surviving route is public and indexable, so the
// noindex path has no callers — reinstate it from git history if a
// wallet-gated route comes back.

export function canonicalUrl(path: string): string {
  return `${SITE_URL}${path}`;
}

export function ogImageUrl(meta: PageMetadata): string {
  return meta.ogImage ?? DEFAULT_OG_IMAGE;
}

export { SITE_URL, SITE_NAME, DEFAULT_OG_IMAGE };
