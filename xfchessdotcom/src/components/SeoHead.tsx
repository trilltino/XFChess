import type { PageMetadata } from '../lib/seo/metadata';
import { canonicalUrl, ogImageUrl, SITE_NAME } from '../lib/seo/metadata';

/** Per-route metadata; build-time prerendering handles zero-JavaScript bots. */
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
