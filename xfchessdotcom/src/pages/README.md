# xfchessdotcom/src/pages

One component per route, registered in [../App.tsx](../App.tsx). Pages are
grouped into subdirectories by what they're relevant to.

## Groups

| Directory | Pages | Flow |
|-----------|-------|------|
| [marketing/](marketing/) | home.tsx, features.tsx | Landing page and the feature list |
| [play/](play/) | play.tsx | Platform download links, resolved from the latest GitHub release |
| [tournaments/](tournaments/) | tournaments.tsx | Live calendar of scheduled events, read from `GET /tournaments` |

The site is deliberately four routes: `/home`, `/play`, `/tournaments`,
`/features`. Anything else falls through the catch-all in App.tsx to `/home`.

The auth, regulatory, and social groups (sign-in, profile, KYC, identity
vault, wallet setup, player lookup, spectate, legal, compliance, anti-cheat,
release notes) and the per-tournament detail/standings/play pages were
removed deliberately, not lost — recover any of them from git history rather
than rewriting from scratch.

## Conventions

- Pages compose [../components/](../components/) and call the backend only through
  [../lib/api/](../lib/api/).
- Each page's imports reach up two levels (`../../components`, `../../lib`, `../../assets`)
  since pages live one directory deeper than before, inside their group folder.
