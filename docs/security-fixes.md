# Security Fixes

Two findings from the 2026-05-26 audit. Both are small, targeted changes.

---

## Fix 1: Gate Swiss mutation endpoints behind `require_api_key`

**Severity:** High  
**File:** `backend/src/infrastructure/router.rs`

### Problem

`swiss_routes()` is mounted without auth middleware. Eight state-mutating endpoints
(mark_absent, withdraw, rejoin, forbidden-pair, manual-pair, result override) are
publicly accessible to any unauthenticated caller.

### Change

Split `swiss_routes()` into a read router and an admin router, or apply the layer
directly to the current mount. Option A (split) is cleaner; Option B is one line.

**Option A — split (preferred)**

In `backend/src/signing/swiss/handlers.rs`, expose two routers:

```rust
pub fn swiss_read_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/current-round", get(get_current_round))
        .route("/{id}/pairings/{round}", get(get_pairings))
        .route("/{id}/standings", get(get_standings))
}

pub fn swiss_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/round", post(start_round))
        .route("/{id}/result", post(record_result))
        .route("/{id}/result", put(override_result))
        .route("/{id}/absent", post(mark_absent))
        .route("/{id}/withdraw", post(withdraw_player))
        .route("/{id}/rejoin", post(rejoin_player))
        .route("/{id}/forbidden-pair", post(add_forbidden_pair))
        .route("/{id}/forbidden-pair", delete(remove_forbidden_pair))
        .route("/{id}/manual-pair", post(add_manual_pairing))
        .route("/{id}/manual-pair", delete(remove_manual_pairing))
}
```

In `backend/src/infrastructure/router.rs`, replace the current mount:

```rust
// Before
.nest("/tournament", swiss_routes())

// After
.nest("/tournament", swiss_read_routes())
.nest("/admin/tournament",
    swiss_admin_routes()
        .layer(middleware::from_fn(require_api_key))
)
```

**Option B — one-liner (quick patch)**

```rust
// router.rs
.nest("/tournament",
    swiss_routes()
        .layer(middleware::from_fn(require_api_key))
)
```

This gates the GET endpoints too, which is acceptable if admin tooling already
sends the API key for all requests.

### Verification

After the change, confirm:

```bash
# Should return 401 / 403
curl -X PUT http://localhost:8080/admin/tournament/1/result \
  -H "Content-Type: application/json" \
  -d '{"round":1,"board":1,"result":"1-0"}'

# Should return 200 with valid key
curl -X PUT http://localhost:8080/admin/tournament/1/result \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $ADMIN_API_KEY" \
  -d '{"round":1,"board":1,"result":"1-0"}'
```

---

## Fix 2: Make `IDENTITY_SALT` a hard startup requirement

**Severity:** Medium  
**File:** `backend/src/signing/config.rs:83`

### Problem

`IDENTITY_SALT` silently falls back to a hardcoded all-ones string if the env var is
not set. Any environment (dev, staging, CI) that omits the variable stores KYC tax-ID
blind indexes derivable from a publicly known salt, enabling offline rainbow-table
deanonymization.

### Change

```rust
// Before (config.rs:83)
identity_salt: env::var("IDENTITY_SALT")
    .unwrap_or_else(|_| "1111111111111111111111111111111111111111111111111111111111111111".to_string()),

// After
identity_salt: env::var("IDENTITY_SALT")
    .expect("IDENTITY_SALT must be set — generate with: openssl rand -hex 32"),
```

This makes `IDENTITY_SALT` consistent with `JWT_SECRET` and `IDENTITY_ENCRYPTION_KEY`,
both of which already use `.expect()`.

### Local dev

Add `IDENTITY_SALT` to `backend/.env` if not present:

```bash
echo "IDENTITY_SALT=$(openssl rand -hex 32)" >> backend/.env
```

And to any CI environment secrets / Docker compose env files that previously relied
on the default.

### Verification

```bash
# Unset the variable — server must refuse to start
IDENTITY_SALT= cargo run --bin backend
# Expected: thread 'main' panicked at 'IDENTITY_SALT must be set ...'
```

---

## Implementation order

| Priority | Fix | Effort |
|----------|-----|--------|
| 1 | Require `IDENTITY_SALT` (config.rs one-liner) | ~2 min |
| 2 | Gate Swiss admin routes (router.rs + handlers.rs split) | ~15 min |

Fix 1 first — it is one line and closes a silent failure mode with no risk of
breaking existing behaviour (production already sets the variable).
