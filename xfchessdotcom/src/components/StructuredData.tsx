import { SITE_URL, SITE_NAME } from '../lib/seo/metadata';

/**
 * JSON-LD structured data. Rendered as a plain inline
 * <script type="application/ld+json">, which is valid anywhere in the
 * document (Google's structured-data parser doesn't require <head>
 * placement), so no hoisting behavior is needed here — unlike SeoHead.
 *
 * Each schema type is its own component so a page only carries the JSON-LD
 * that's actually about it (see js_handyman/food_man/backend/src/seo.rs's
 * `render_index`: the home page gets its business schema, /checkout gets
 * none — resist the temptation to stamp the same block on every page).
 */

function JsonLd({ data }: { data: Record<string, unknown> }) {
  return (
    <script type="application/ld+json">
      {JSON.stringify(data)}
    </script>
  );
}

/** Organization schema — mount once, site-wide (e.g. in the root layout).
 * Called out in 2026 research as specifically high-value for AI
 * answer-engine trust/citation, not just classic rich snippets: a clear,
 * consistent Organization block with real sameAs links is what lets an LLM
 * verify "is this a real, identifiable source." */
export function OrganizationSchema() {
  return (
    <JsonLd
      data={{
        '@context': 'https://schema.org',
        '@type': 'Organization',
        name: SITE_NAME,
        url: SITE_URL,
        logo: `${SITE_URL}/og-image.png`,
        sameAs: [
          'https://twitter.com/xfchess',
          'https://github.com/xfchess',
          'https://youtube.com/xfchess',
        ],
      }}
    />
  );
}

/** VideoGame schema — mount on /home and /play only. */
export function VideoGameSchema() {
  return (
    <JsonLd
      data={{
        '@context': 'https://schema.org',
        '@type': 'VideoGame',
        name: SITE_NAME,
        description:
          'Competitive 3D chess with ranked matchmaking, wagered PvP, and Swiss-format tournaments, built on Solana.',
        url: SITE_URL,
        genre: ['Strategy', 'Board Game'],
        gamePlatform: ['Windows', 'macOS', 'Linux'],
        operatingSystem: ['Windows', 'macOS', 'Linux'],
        applicationCategory: 'Game',
      }}
    />
  );
}

/** SportsEvent schema — mount on /tournament/:id with the real tournament's
 * own data. `startDate` should be an ISO 8601 string; omit fields that
 * aren't known rather than fabricate placeholder values. */
export function TournamentEventSchema({
  id,
  name,
  startDate,
  status,
}: {
  id: string | number;
  name: string;
  startDate?: string;
  status?: 'scheduled' | 'active' | 'completed' | 'cancelled';
}) {
  const eventStatusMap: Record<string, string> = {
    scheduled: 'https://schema.org/EventScheduled',
    active: 'https://schema.org/EventScheduled',
    completed: 'https://schema.org/EventScheduled',
    cancelled: 'https://schema.org/EventCancelled',
  };

  return (
    <JsonLd
      data={{
        '@context': 'https://schema.org',
        '@type': 'SportsEvent',
        name,
        url: `${SITE_URL}/tournament/${id}`,
        ...(startDate ? { startDate } : {}),
        ...(status ? { eventStatus: eventStatusMap[status] } : {}),
        eventAttendanceMode: 'https://schema.org/OnlineEventAttendanceMode',
        location: {
          '@type': 'VirtualLocation',
          url: `${SITE_URL}/tournament/${id}`,
        },
        organizer: {
          '@type': 'Organization',
          name: SITE_NAME,
          url: SITE_URL,
        },
      }}
    />
  );
}

/** BreadcrumbList schema — mount on the /tournament/:id/* page family. */
export function TournamentBreadcrumbSchema({
  id,
  name,
  currentLabel,
}: {
  id: string | number;
  name: string;
  currentLabel: string;
}) {
  return (
    <JsonLd
      data={{
        '@context': 'https://schema.org',
        '@type': 'BreadcrumbList',
        itemListElement: [
          { '@type': 'ListItem', position: 1, name: 'Tournaments', item: `${SITE_URL}/tournaments` },
          { '@type': 'ListItem', position: 2, name, item: `${SITE_URL}/tournament/${id}` },
          { '@type': 'ListItem', position: 3, name: currentLabel },
        ],
      }}
    />
  );
}
