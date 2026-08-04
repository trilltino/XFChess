# Fork provenance

Tracked per `docs/PRE_MAINNET_E2E_PLAN.md` §4.1: this crate arrived
already-vendored (no local history of the import itself — see "How this was
reconstructed" below), so this file is the best-effort record going forward.
Update it whenever a local change is made here, rather than letting future
readers re-derive provenance from scratch again.

## Upstream origin

- Upstream: [`n0-computer/iroh-gossip`](https://github.com/n0-computer/iroh-gossip)
- Best available anchor: **v0.101.0** (`CHANGELOG.md:5-9`, 2026-06-15), whose
  sole entry is *"Update to iroh 1.0"*
  ([n0-computer/iroh-gossip#148](https://github.com/n0-computer/iroh-gossip/issues/148),
  upstream commit `c190a7902f8e215f5ceb8cf1680350d1a2ae4c9d`). This is an
  approximation, not a pinned checkout — see caveat below.
- `Cargo.toml` sets `version.workspace = true` (currently `0.1.0`, the
  XFChess workspace version), overwriting whatever upstream version string
  the vendored snapshot originally had. This is itself a local change (see
  "Known local changes" below).

## How this was reconstructed

`git log --diff-filter=A` on this crate's current path
(`crates/zarathustra_net/iroh-gossip/`) bottoms out at commit `67a1fbe85`
("refactor: rename crates/networking -> crates/zarathustra_net"), which is a
pure directory rename — it moved an already-vendored crate, it didn't
introduce it. No earlier commit in this repository's history adds the crate
from scratch, so there is no local commit boundary to diff against upstream
by `git log` alone. The v0.101.0 anchor above comes from cross-referencing
`CHANGELOG.md` (which is upstream's own auto-generated `git-cliff` history,
carried along with the vendored snapshot, not a local diff) against the
`iroh = "1"` / `iroh-base = "1"` major-version dependencies declared in this
crate's `Cargo.toml` — v0.101.0 is upstream's first release built against
`iroh` 1.0 rather than a `1.0.0-rc.*`, which matches this fork's own pins
(`Cargo.toml:50,62,101`, now inheriting the workspace's exact `iroh = "=1.0.3"`
pin per §4.2 below).

**Caveat:** this is circumstantial (dependency-version matching), not a
pinned upstream commit/tag checked out locally. If a real diff against
upstream v0.101.0 is ever done and turns out to disagree with this anchor,
trust the diff and correct this file.

## Known local changes

None found. A targeted search of `src/**/*.rs` for XFChess-specific markers
(project name, `PATCH`/`CUSTOM`/`XXX` comments) returned no hits, and no
`FORK_NOTES`/`PATCH`/`VENDOR` file existed before this one. The README's
"may have custom patches" (`README.md:11,49`) should be read as "unverified,"
not "verified present" — nothing distinguishing this from an unmodified
v0.101.0 snapshot was found during this pass.

The only concrete divergence identified is metadata-level, not
protocol/logic-level:
- `Cargo.toml`'s `version` field was overwritten to track the XFChess
  workspace version (`0.1.0`) instead of upstream's own version string.
- Until this pass (§4.2, `docs/PRE_MAINNET_E2E_PLAN.md`), `iroh`/`iroh-base`
  were pinned locally as `"1"` (loose) rather than inheriting the workspace's
  exact `=1.0.3` pin — now fixed to inherit.

## If you make a local change here

Add a dated entry below. Include what changed and why (not just "patched
X"), so a future upstream-sync attempt knows exactly what to re-apply or
reconcile.

- *(none yet)*
