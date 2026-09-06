/** Per-route SEO metadata registry with explicit public-route indexing. */

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

export function canonicalUrl(path: string): string {
  return `${SITE_URL}${path}`;
}

export function ogImageUrl(meta: PageMetadata): string {
  return meta.ogImage ?? DEFAULT_OG_IMAGE;
}

export { SITE_URL, SITE_NAME, DEFAULT_OG_IMAGE };
