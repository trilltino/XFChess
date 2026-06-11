# Plan: Lichess Link Button — UI Visibility & Profile Explorer

## Problem

1. **ProfileViewer.tsx** — The "Link Lichess Account" button is buried inside `{!loading && profile && (…)}`. If the profile hasn't loaded or the user hasn't created one yet, the button never renders. There's also no "already linked" state — it shows the link button even if Lichess is already connected, with no display of the linked username or ratings.

2. **Players.tsx (Profile Explorer)** — Zero Lichess integration. When you search a player by username or pubkey, there's no Lichess section shown, no link button for own profile, and no ratings display for other players.

---

## Deliverables

| # | File | Change |
|---|------|--------|
| 1 | `web-solana/src/components/LichessLinkCard.tsx` | New shared component — two states: linked / unlinked |
| 2 | `web-solana/src/pages/ProfileViewer.tsx` | Use `LichessLinkCard`, add to checklist row, always visible when wallet connected |
| 3 | `web-solana/src/pages/Players.tsx` | Add Lichess section to searched profile — link button if own profile, ratings display if other player's |

---

## Task 1 — Create `LichessLinkCard` component

**File:** `web-solana/src/components/LichessLinkCard.tsx`

Props:
```ts
interface LichessLinkCardProps {
  walletPubkey: string | null;        // null = viewing someone else's profile
  lichessUsername?: string;           // from profile.data.lichessUsername
  lichessBlitz?: number;              // centiscale → divide by 100
  lichessRapid?: number;
  lichessBullet?: number;
  lichessVerified?: boolean;
}
```

**Linked state** (lichessUsername is truthy):
- Show Lichess knight icon + username in a green-tinted card
- Show blitz / rapid / bullet ratings as small stat chips
- Show "Verified" badge if `lichessVerified === true`
- No button (already done)

**Unlinked state** (no lichessUsername):
- If `walletPubkey` is provided (own profile): show "Link Lichess Account" button + subtitle "Seed your ELO from your verified Lichess rating"
- If `walletPubkey` is null (viewing other player): show "Lichess not linked" in muted text — no button
- Button handler: dynamic import of `initLichessLink`, popup window (same logic as current ProfileViewer)

---

## Task 2 — Fix ProfileViewer.tsx

### 2a — Add to verification checklist

After the existing `ChecklistRow` entries (lines 513–517), add:

```tsx
<ChecklistRow
  label="Lichess linked"
  ok={!!(profile?.data.lichessUsername)}
  action={
    !profile?.data.lichessUsername ? (
      <button className="btn-small" onClick={handleLichessLink}>Link</button>
    ) : undefined
  }
/>
```

Where `handleLichessLink` is extracted from the inline button handler currently at line 569.

### 2b — Replace inline button with `LichessLinkCard`

Remove the current `{/* Link Lichess OAuth */}` div (lines 566–587).

Replace with:

```tsx
<LichessLinkCard
  walletPubkey={wallet.publicKey?.toBase58() ?? null}
  lichessUsername={profile?.data.lichessUsername}
  lichessBlitz={profile?.data.lichessBlitz}
  lichessRapid={profile?.data.lichessRapid}
  lichessBullet={profile?.data.lichessBullet}
  lichessVerified={profile?.data.lichessVerified}
/>
```

Place this **outside** the `{!loading && profile && (…)}` guard — it should render whenever the wallet is connected, whether or not a profile exists (showing the unlinked state encourages first-time users to link).

---

## Task 3 — Add Lichess section to Players.tsx (Profile Explorer)

After the stats block (line 148, after the `</div>` closing `connected-stats`), add:

```tsx
<div style={{ marginTop: 20 }}>
  <LichessLinkCard
    walletPubkey={
      wallet.publicKey &&
      profile.authority === wallet.publicKey.toBase58()
        ? wallet.publicKey.toBase58()
        : null
    }
    lichessUsername={profile.data.lichessUsername}
    lichessBlitz={profile.data.lichessBlitz}
    lichessRapid={profile.data.lichessRapid}
    lichessBullet={profile.data.lichessBullet}
    lichessVerified={profile.data.lichessVerified}
  />
</div>
```

The `walletPubkey` ternary means:
- **Own profile**: wallet pubkey passed in → "Link Lichess" button shows if not linked
- **Other player**: null passed in → read-only Lichess display (or "not linked" text)

---

## Task 4 — Verify profile.data fields

Check `web-solana/src/lib/anchor_client.ts` to confirm `lichessUsername`, `lichessBlitz`, `lichessRapid`, `lichessBullet`, `lichessVerified` are decoded from the on-chain `PlayerProfile` account. If not, add them to the profile fetch mapping.

---

## Acceptance criteria

- [ ] Wallet connected + profile exists + Lichess not linked → Link button visible in ProfileViewer
- [ ] Wallet connected + profile exists + Lichess linked → Username + ratings shown, no button
- [ ] Verification checklist shows "Lichess linked" row with green tick or "Link" CTA
- [ ] Players.tsx search result for own pubkey → Lichess section with link button if not linked
- [ ] Players.tsx search result for other player → Lichess ratings if linked, muted "not linked" if not
- [ ] No duplicate popup windows on repeated clicks (disable button while popup is open)
