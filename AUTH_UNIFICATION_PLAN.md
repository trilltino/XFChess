# Auth Unification Plan — XFChess

## Problem Summary

Three surfaces each do auth differently and don't share state:

| Surface | Auth method | Token storage | Problem |
|---|---|---|---|
| `web-solana` (port 5173) | Solflare adapter / email | `xfchess_token`, `xfchess_wallet` | Works, but KYC/email checklist incomplete |
| `tauri/wallet-ui` (port 7454) | Direct Phantom/Solflare | `xfchess_token`, `xfchess_wallet_pubkey` | Starts from scratch every launch; ignores existing session |
| `xfchess.exe` (Rust/Bevy) | Reads from env / HTTP call | env vars / HTTP | Receives token via `xfchess://launch` protocol URL |

**Immediate bugs:**
1. Tauri ignores existing `xfchess_token` on startup → forces re-auth for every session
2. `find_user_by_wallet` silently fails because `password_hash` column was missing (fixed by migration 007)
3. `ProfileStep` in Tauri asks for a new handle, ignoring the username already in the backend
4. localStorage key mismatch: web uses `xfchess_wallet`, Tauri uses `xfchess_wallet_pubkey`

---

## Canonical localStorage Keys (both surfaces must use these)

```
xfchess_token          — JWT from backend
xfchess_username       — display username
xfchess_wallet         — Solana wallet pubkey (base58)
xfchess_email          — email if email-auth used (optional)
```

> Remove `xfchess_wallet_pubkey` from Tauri — use `xfchess_wallet` everywhere.

---

## Canonical Auth Flow (both surfaces)

```
1. MOUNT
   └─ Is xfchess_token + xfchess_wallet set?
       ├─ YES → GET /api/auth/me (validate token)
       │         ├─ 200 → skip to VERIFIED state (game ready)
       │         └─ 401 → clear keys, go to CONNECT
       └─ NO  → go to CONNECT

2. CONNECT (first-time or expired session)
   └─ User connects wallet (Phantom / Solflare / hot-key)
       └─ GET /api/auth/check-wallet/{pubkey}
           ├─ registered → sign login message → POST /api/auth/login → JWT
           └─ not registered → sign register message → POST /api/auth/register → JWT
               (username defaults to first 8 chars of pubkey; user can update later)

3. POST-AUTH (after JWT issued)
   └─ Store: xfchess_token, xfchess_username, xfchess_wallet
   └─ Call POST /api/game/launch { pubkey, username, token }
   └─ Show VERIFIED checklist:
       ✓ Wallet connected
       ? Email registered   → optional: prompt to add (POST /api/auth/add-email)
       ? KYC verified       → link to /kyc on web-solana or embedded KYC modal
       ? Wagered eligible   → derived from KYC status

4. LAUNCH
   └─ Deep-link: xfchess://launch?pubkey=...&username=...&token=...
```

---

## Backend Changes Required

### New endpoint: `GET /api/auth/me`
Validates the Bearer token and returns the caller's profile.
```
Headers: Authorization: Bearer <jwt>
Response 200: { wallet, username, email, kyc_status }
Response 401: { error: "Invalid or expired token" }
```

### New endpoint: `POST /api/auth/add-email`
Allows a wallet-auth'd user to attach an email (no password required yet).
```
Body: { email }
Headers: Authorization: Bearer <jwt>
Response 200: { ok: true }
```

---

## Tauri wallet-ui Changes

### `Onboarding` root — session resume
```typescript
useEffect(() => {
  const token = localStorage.getItem("xfchess_token");
  const wallet = localStorage.getItem("xfchess_wallet");
  if (token && wallet) {
    fetch(`${API_BASE}/api/auth/me`, {
      headers: { Authorization: `Bearer ${token}` }
    }).then(r => r.ok ? r.json() : null).then(user => {
      if (user) {
        setUsername(user.username);
        setPubkey(wallet);
        handleGameLaunch(wallet, false, user.username);
        setStep("splash");
      } else {
        localStorage.removeItem("xfchess_token");
        setStep("consent");
      }
    }).catch(() => setStep("consent")).finally(() => setReady(true));
  } else {
    setReady(true);
  }
}, []);
```

### `WalletStep` — standardise key name
- Replace `xfchess_wallet_pubkey` with `xfchess_wallet`

### `ProfileStep` — use backend username, don't re-ask
- On mount, call `GET /api/auth/check-wallet/{pubkey}`
- Pre-fill handle from `data.username`
- Show email-addition prompt if `!localStorage.getItem("xfchess_email")`
- Show KYC prompt if `kyc_status === "none"`

---

## web-solana Changes

### `SignIn.tsx` ProfileStep — same check-wallet call already exists
- Already calls `check-wallet` to detect registration ✓
- After wallet login, show post-auth checklist (already shown in Verification component)
- Email prompt: if email MISSING, show inline form calling `POST /api/auth/add-email`

### localStorage key normalisation
- Replace `xfchess_wallet_pubkey` with `xfchess_wallet` everywhere in `Play.tsx`, `Kyc.tsx`, `ProfileViewer.tsx`

---

## KYC Flow

KYC is handled at `/kyc` on web-solana (`Kyc.tsx`). After completing KYC:
- Backend sets `kyc_status = "approved"` in `users_v2`
- Verification checklist re-fetches and shows ✓

Both surfaces link to `http://localhost:5173/kyc` for KYC completion.
In a future phase this can be an embedded iframe modal.

---

## Implementation Order

- [x] Fix migration 007 — add `password_hash` column (done)
- [ ] Add `GET /api/auth/me` backend endpoint
- [ ] Add `POST /api/auth/add-email` backend endpoint  
- [ ] Tauri: session-resume on mount (check token → skip flow if valid)
- [ ] Tauri: normalise key `xfchess_wallet_pubkey` → `xfchess_wallet`
- [ ] Tauri: `ProfileStep` pre-fill username from backend, add email prompt
- [ ] web-solana: normalise key `xfchess_wallet_pubkey` → `xfchess_wallet`
- [ ] web-solana: email prompt in Verification checklist calls `add-email`
