# XFChess Revenue Model

## The Core Principle

**Entry fees = prize pool + platform cut. No external funding needed.**

Players pay in. 83% goes back out as prizes. 17% (50p per player) is the platform fee.
The contract enforces the split automatically — the operator cannot take more.

---

## Pricing Structure

£3 entry across all bracket sizes. Prize scales with turnout.

| Players | Entry | Prize Pool (83%) | Platform Cut | Winner (60%) | 2nd (30%) | 3rd (10%) |
|---|---|---|---|---|---|---|
| 8 | £3 | £20 | £4 | £12 | £6 | £2 |
| 16 | £3 | £40 | £8 | £24 | £12 | £4 |
| 32 | £3 | £80 | £16 | £48 | £24 | £8 |
| 64 | £3 | £160 | £32 | £96 | £48 | £16 |
| 128 | £3 | £320 | £64 | £192 | £96 | £32 |
| 256 | £3 | £640 | £128 | £384 | £192 | £64 |

Player expected value: **£2.50 on a £3 entry** — consistent at every bracket size.

---

## Legal Positioning

- Pure skill game (chess) — not gambling anywhere
- Platform takes a transparent, fixed service fee — not a rake on outcomes
- Prize pool is locked in the smart contract at registration — operator cannot withhold it
- Winner claims payout themselves directly to their wallet
- Same legal structure as poker rooms, fantasy sports, esports platforms

---

## The £500 Seed

- Not used for operations — entry fees are self-funding from day one
- Sits as a reserve for edge cases (Solana outage, disputed result, manual refund)
- Only depleted if a tournament runs with a guaranteed fixed prize and doesn't fill
- With the 83/17 model and no fixed guarantee, seed is never touched

---

## Daily Revenue at Different Stages

### Early (8-player only, 3/day)
| Week | Avg players | Daily profit | Week total |
|---|---|---|---|
| Week 1 | 6 (refunded, doesn't fill) | £0 | £0 |
| Week 2 | 8 | £4.50 | £31 |
| Week 3 | 8–16 mixed | £12 | £84 |
| Week 4 | 16 consistently | £32 | £224 |

**Month 1: ~£340**

### Growing (mixed brackets, 9/day)
| Bracket | Count/day | Cut each | Daily |
|---|---|---|---|
| 8-player | 4 | £4 | £16 |
| 16-player | 3 | £8 | £24 |
| 32-player | 2 | £16 | £32 |
| **Total** | **9** | | **£72/day → £2,160/month** |

### At Scale (100 tournaments/day)
| Bracket | Count/day | Cut each | Daily |
|---|---|---|---|
| 8-player | 60 | £4 | £240 |
| 16-player | 30 | £8 | £240 |
| 32-player | 10 | £16 | £160 |
| **Total** | **100** | | **£640/day → £19,200/month** |

Requires ~400 daily active users playing ~3 tournaments each. Achievable with a modest community.

---

## Growth Timeline

| Month | State | MRR |
|---|---|---|
| 1 | 8-player brackets, building audience | £340 |
| 2 | 16-player filling, 2 daily 32-player evenings | £1,920 |
| 3 | Mixed schedule running smoothly | £2,160 |
| 6 | 5+ tournaments/day at 32-player avg | £5,000+ |
| 12–18 | 100+ tournaments/day, 400 DAU | £10,000–£20,000 |

Profitable from **week 2** (first filled 8-player bracket). £500 seed never needed for operations.

---

## Suggested Daily Schedule (Established)

| Time | Format | Players | Your cut |
|---|---|---|---|
| 7am | 8-player bullet | 8 | £4 |
| 9am | 8-player bullet | 8 | £4 |
| 12pm | 16-player blitz | 16 | £8 |
| 2pm | 8-player blitz | 8 | £4 |
| 4pm | 16-player blitz | 16 | £8 |
| 6pm | 32-player blitz | 32 | £16 |
| 8pm | 32-player main event | 32 | £16 |
| 10pm | 16-player blitz | 16 | £8 |
| Weekend | 64-player championship | 64 | £32 |

---

## Why This Works

- **No external prize funding** — entry fees cover prizes and platform cut
- **Fair odds at every size** — player EV stays constant regardless of field
- **Contract pays winners** — not a person, not PayPal, not a bank transfer
- **Near-zero infra cost** — games run on MagicBlock Ephemeral Rollup
- **Scales without overhead** — 100 simultaneous tournaments costs the same as 1

---

## The Pitch

> "Show up, pay £3, play chess, get paid instantly if you win."

No verification hoops. No disputed payouts. No tournament director who ghosts you.
The prize is locked on-chain the moment registration opens. The winner claims it themselves.

That is the product.
