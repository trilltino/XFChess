# External ELO Integration Plan: Lichess

Connects established platform ratings (Lichess) into the on-chain XFChess PlayerProfile, enabling accurate seeding and fair matchmaking from day one.

---

## 1. Overview

**Problem:** New players start at 1200 ELO on-chain. A 2200 FIDE-rated player matched against a true 1200 beginner is unfair and damages trust.

**Solution:** Let players cryptographically link their Lichess account. The backend fetches their public ratings, verifies ownership, and seeds the on-chain profile. Post-seeding, the on-chain K=32 system takes over.

**Scope:**
- Lichess (OAuth 2.0 + public API, free)
- Rating types: Blitz and Rapid (configurable per tournament type)

---

## 2. On-Chain Changes

### 2.1 Extend `PlayerProfile`

```rust
// programs/xfchess-game/src/state/player_profile.rs
#[account]
#[derive(InitSpace, Default)]
pub struct PlayerProfile {
    // ... existing fields ...
    #[max_len(20)]
    pub username: String,
    pub username_set: bool,

    // ── External platform linkage ──
    #[max_len(30)]
    pub lichess_username: String,
    pub lichess_verified: bool,
    pub lichess_blitz: u32,      // stored in centiscale ×100
    pub lichess_rapid: u32,
    pub lichess_bullet: u32,
    pub lichess_last_sync: i64,  // Unix timestamp

    pub external_elo_source: u8, // 0=none, 1=lichess
    pub seeded_from_external: bool,
}
```

**Account space impact:**
- Current `PlayerProfile::INIT_SPACE` ≈ 280 bytes
- New fields add ≈ 120 bytes → ~400 bytes total
- Rent increase: negligible (~0.003 SOL)

### 2.2 New On-Chain Instruction: `LinkExternalElo`

```rust
// programs/xfchess-game/src/account_ix/link_external_elo.rs
#[derive(Accounts)]
#[instruction(
    platform: u8,        // 1 = Lichess
    username: String,
    rating: u32,         // centiscale rating
    challenge_sig: [u8; 64], // Ed25519 or ECDSA signature proving ownership
)]
pub struct LinkExternalElo<'info> {
    #[account(mut, seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
    pub player_profile: Account<'info, PlayerProfile>,
    pub player: Signer<'info>,
    /// CHECK: KYC / Linking authority — the VPS signs off on verified links
    #[account(signer, address = crate::constants::link_authority::ID)]
    pub link_authority: AccountInfo<'info>,
}
```

**Handler logic:**
1. Verify `player_profile.authority == player.key()`
2. Verify `link_authority` signature (prevents spoofed linking)
3. Store username, rating, `verified = true`, timestamp
4. If `!seeded_from_external`, copy rating into `elo_rating` and set `seeded_from_external = true`
5. Set `external_elo_source = platform`

**Key point:** The on-chain program does NOT call external APIs. It only accepts a VPS-signed attestation. The backend does the actual API verification.

---

## 3. Backend API Flow

### 3.1 Lichess Link Flow

```
Player opens "Link Lichess" in UI
  → Backend generates nonce: "xfchess_link:{pubkey}:{timestamp}:{nonce}"
  → Player pastes nonce into Lichess profile bio (or DM to a bot)
  → Backend polls Lichess API /api/user/{username} every 10s for 5 min
  → Once bio contains nonce, ownership is proven
  → Backend fetches ratings: /api/user/{username}/rating-history
  → Backend signs LinkExternalElo IX with link_authority keypair
  → Backend submits on-chain
  → Returns tx signature to player
```

**Lichess API endpoints:**
- `GET https://lichess.org/api/user/{username}` → profile + current ratings
- `GET https://lichess.org/api/user/{username}/rating-history/{perf}` → time series

**Rate limits:** Lichess allows ~20 req/s for authenticated apps. Use a single OAuth app token.

### 3.2 New Backend Routes

```rust
// backend/src/signing/routes/external_elo.rs

POST /api/external-elo/link/start
  Body: { pubkey, username }
  Response: { link_id, nonce, expires_at }

POST /api/external-elo/link/confirm
  Body: { link_id }
  Response: { tx_signature, platform, rating }
  // Backend polls, verifies, submits on-chain

GET /api/external-elo/status/{pubkey}
  Response: {
    lichess: { username, verified, blitz, rapid, last_sync } | null,
    on_chain_elo: number,
    seeded_from_external: bool
  }

POST /api/external-elo/sync
  Body: { pubkey, signature, timestamp }  // wallet-signed
  Response: { updated: bool, old_elo, new_elo }
  // Forces a re-sync of external ratings → on-chain
```

---

## 4. ELO Seeding & Sync Logic

### 4.1 Initial Seeding (First Link)

When a player links an external account for the first time:

```python
# Pseudocode for backend seeding decision
if profile.seeded_from_external:
    return  # Already seeded, don't overwrite

# Select rating based on game type preference
if tournament_type == "blitz":
    external_rating = lichess_blitz
elif tournament_type == "rapid":
    external_rating = lichess_rapid
else:
    external_rating = max(lichess_blitz, lichess_rapid)

# Clamp to valid range
external_rating = clamp(external_rating, 400, 3200)

# Convert to centiscale for on-chain
on_chain_elo = external_rating * 100.0
```

**Platform bias adjustment (optional):**
| Platform | Rating Type | Bias vs FIDE |
|----------|------------|--------------|
| Lichess Blitz | Blitz | ~+50 |
| Lichess Rapid | Rapid | ~+30 |

These are configurable constants in the backend.

### 4.2 Periodic Re-sync (Every 7 Days)

```python
# Weekly background task
for each linked profile where now - last_sync > 7 days:
    fetch_external_rating()
    
    # If external rating drifted > 100 ELO from on-chain
    if abs(external - on_chain/100) > 100:
        # Create a "rating adjustment" proposal
        # Player must approve via wallet signature
        # Or: auto-adjust if drift > 200 (indicates sandbagging/boosting)
        if drift > 200:
            flag_for_manual_review()
        else:
            queue_adjustment()
```

**Policy:** We never silently overwrite on-chain ELO. External ratings inform matchmaking filtering and tournament banding, but on-chain K=32 is the ground truth for the platform.

### 4.3 Matchmaking Integration

```rust
// backend/src/signing/routes/matchmaking/handlers.rs

// In the join handler, enhance ELO lookup:
let cached_elo = state.elo_cache.get_elo(&req.pubkey).await?;

// If player has external ratings but hasn't played on-chain yet,
// use external for initial matchmaking
let effective_elo = if cached_elo.ranked_games == 0 && cached_elo.seeded_from_external {
    // Use the higher of on-chain (1200 default) or external
    cached_elo.elo_rating.max(cached_elo.external_rapid.max(cached_elo.external_blitz))
} else {
    cached_elo.elo_rating  // On-chain K=32 rating
};
```

---

## 5. Anti-Fraud & Security

### 5.1 Ownership Verification

| Platform | Method | Trust Level |
|----------|--------|-------------|
| Lichess | Bio nonce (public profile) | High — requires account access |
| Lichess (alt) | OAuth 2.0 token exchange | Very High — but requires UI redirect |

**Recommended:** Start with bio-nonce. Upgrade to OAuth later.

### 5.2 Rating Manipulation Defenses

1. **Account age gate:** Reject accounts created < 30 days ago (configurable)
2. **Games played gate:** Reject accounts with < 20 rated games in the relevant time control
3. **Rating stability:** Reject if rating variance (last 10 games) > 200 points
4. **One-wallet-one-platform:** A Lichess account can only link to one wallet. Store mapping in backend DB.
5. **Cooldown on re-linking:** 30-day cooldown before unlinking and re-linking a new account
6. **Human review threshold:** Drift > 300 ELO between platforms triggers manual review queue

### 5.3 Sandbagging Detection

If a player's on-chain performance exceeds their external rating by > 400 ELO after 20 games:
- Flag for anti-cheat review (Stockfish analysis already queued)
- Possible explanations: external account underrated, or engine use on XFChess

---

## 6. Database Schema (Backend)

```sql
-- SQLite migration: add to existing sessions.db or new dedicated table

CREATE TABLE external_elo_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey TEXT NOT NULL,
    platform TEXT NOT NULL CHECK(platform IN ('lichess')),
    username TEXT NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0,  -- bool
    blitz_rating INTEGER,
    rapid_rating INTEGER,
    bullet_rating INTEGER,
    games_count INTEGER,  -- rated games on platform
    account_created_at INTEGER,  -- Unix timestamp
    linked_at INTEGER NOT NULL,
    last_sync_at INTEGER,
    on_chain_tx TEXT,  -- transaction signature
    UNIQUE(pubkey, platform),
    UNIQUE(platform, username)  -- one username per platform globally
);

CREATE TABLE external_elo_sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey TEXT NOT NULL,
    platform TEXT NOT NULL,
    old_rating INTEGER,
    new_rating INTEGER,
    sync_type TEXT NOT NULL CHECK(sync_type IN ('initial', 'weekly', 'manual')),
    synced_at INTEGER NOT NULL,
    on_chain_tx TEXT
);

CREATE INDEX idx_external_elo_pubkey ON external_elo_links(pubkey);
CREATE INDEX idx_external_elo_platform_username ON external_elo_links(platform, username);
```

---

## 7. Implementation Phases

### Phase 1 — Core Linking (Week 1)
- [ ] Extend `PlayerProfile` on-chain (+ migration for existing accounts)
- [ ] Add `LinkExternalElo` instruction + handler
- [ ] Add `link_authority` constant (new Keypair, stored in backend .env)
- [ ] Backend: `POST /api/external-elo/link/start` + `/confirm`
- [ ] Lichess bio-nonce verification
- [ ] Deploy devnet, test with 2-3 real accounts

### Phase 2 — Seeding & Matchmaking (Week 2)
- [ ] Integrate external rating into `EloCache` fetch logic
- [ ] Update matchmaking `join` handler to use effective ELO
- [ ] Add `GET /api/external-elo/status/{pubkey}`
- [ ] Frontend: "Link Account" UI component
- [ ] Frontend: display external ratings on profile page
- [ ] Anti-fraud gates (account age, games played)

### Phase 3 — Sync & Maintenance (Week 3)
- [ ] Background task: weekly re-sync of all linked accounts
- [ ] Drift detection + adjustment queue
- [ ] Manual review dashboard (admin endpoint)
- [ ] Sandbagging detection integration with anti-cheat
- [ ] Platform bias calibration (collect data, adjust constants)

### Phase 4 — Polish (Week 4)
- [ ] Lichess OAuth 2.0 (upgrade from bio-nonce)
- [ ] Rate-limiting and retry logic for external APIs
- [ ] Monitoring: alert if sync success rate drops below 95%
- [ ] Documentation for players (how to link, why it matters)

---

## 8. Cost Analysis

| Component | One-time | Monthly |
|-----------|----------|---------|
| On-chain account rent increase | ~0.003 SOL/link | — |
| Link transaction fees | ~0.000005 SOL | — |
| Weekly sync TX fees | — | ~0.001 SOL / 1000 players |
| Backend API polling | — | Negligible (≤20 req/min at scale) |
| **Total per 1000 linked players** | ~£0.10 | ~£0.02 |

---

## 9. Open Questions

1. **Should we allow multiple platforms per wallet?** (Yes — take highest rating)
2. **What happens when a player unlinks?** Keep on-chain ELO, but mark `seeded_from_external = false` for future re-linking
3. **Tournament-specific banding?** Use external rating for initial tournament placement, then on-chain rating for subsequent
4. **Bot detection on external platforms?** Not in scope — we trust the platform's own detection. Flag only if XFChess performance diverges wildly.

---

## 10. Files to Touch

### On-chain (`programs/xfchess-game/`)
- `src/state/player_profile.rs` — add external fields
- `src/account_ix/link_external_elo.rs` — NEW instruction
- `src/lib.rs` — register new instruction
- `src/constants.rs` — add `link_authority` pubkey

### Backend (`backend/src/signing/`)
- `routes/external_elo.rs` — NEW routes module
- `mod.rs` — add `external_elo` to exports, integrate into `build_router`
- `elo_cache.rs` — enhance fetch to consider external ratings
- `routes/matchmaking/handlers.rs` — use effective ELO in join handler

### Frontend (`web-solana/`)
- New "Link External Account" settings page
- Profile page shows external ratings
- Tournament entry shows effective ELO

### Infrastructure
- `.env` — add `LINK_AUTHORITY_KEY`, `LICHESS_OAUTH_TOKEN`
- `deploy/backend/xfchess-backend.service` — no changes needed
