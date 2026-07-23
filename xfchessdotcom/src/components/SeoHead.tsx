import type { PageMetadata } from '../lib/seo/metadata';
import { canonicalUrl, ogImageUrl, SITE_NAME } from '../lib/seo/metadata';

/**
 * Per-route <head> metadata. Uses React 19's native support for rendering
 * <title>/<meta>/<link> anywhere in the tree — React hoists them into
 * <head> itself, so no react-helmet-async (or any extra dependency) is
 * needed here. See docs/plans/web-solana-seo-sitemap-plan.md §4.2/§4.3.
 *
 * Important limitation this component does NOT solve on its own: this is
 * still a client-side render, so it only helps real browsers and Google's
 * second-wave JS rendering pass — zero-JS bots (social link-preview bots,
 * many non-Google crawlers) never see tags injected this way. That gap is
 * covered separately by the build-time prerendering step (plan §5/Phase 3),
 * which bakes the same tags into real static HTML per public route.
 */
export function SeoHead({ meta }: { meta: PageMetadata }) {
  const url = canonicalUrl(meta.path);
  const image = ogImageUrl(meta);

  if (meta.noindex) {
    return (
      <>
        <title>{meta.title}</title>
        <meta name="robots" content="noindex, nofollow" />
      </>
    );
  }

  return (
    <>
      <title>{meta.title}</title>
      <meta name="description" content={meta.description} />
      <meta name="robots" content="index, follow" />
      <link rel="canonical" href={url} />

      <meta property="og:type" content="website" />
      <meta property="og:site_name" content={SITE_NAME} />
      <meta property="og:title" content={meta.title} />
      <meta property="og:description" content={meta.description} />
      <meta property="og:url" content={url} />
      <meta property="og:image" content={image} />
      <meta property="og:image:width" content="1200" />
      <meta property="og:image:height" content="630" />

      <meta name="twitter:card" content="summary_large_image" />
      <meta name="twitter:title" content={meta.title} />
      <meta name="twitter:description" content={meta.description} />
      <meta name="twitter:image" content={image} />
    </>
  );
}
