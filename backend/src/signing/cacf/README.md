# backend/src/signing/cacf

Country-specific compliance rules (CACF — Crypto Asset Compliance Framework) applied
**before** any wager transaction is built: KYC requirements, tax-ID validation, and
reporting duties per jurisdiction.

## Files

| File | Jurisdiction |
|------|--------------|
| [uk.rs](uk.rs) | United Kingdom |
| [brazil.rs](brazil.rs) | Brazil (CPF validation) |
| [germany.rs](germany.rs) | Germany |
| [canada.rs](canada.rs) | Canada |
| [types.rs](types.rs) | Shared verdict/requirement types |

## Adding a jurisdiction

Add a new file implementing the same check interface, register it in
[mod.rs](mod.rs) — the wager routes pick it up by country code. No other module
should embed country logic.
