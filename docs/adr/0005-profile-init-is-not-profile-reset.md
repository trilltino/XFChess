# ADR 0005: Profile Init Is Not Profile Reset

## Status

Accepted.

## Decision

`init_profile` may create a profile or update identity fields, but it must preserve existing gameplay, verification, and external-link fields.

## Consequences

- Re-running profile setup cannot wipe ELO, stats, streaks, or linked ratings.
- Identity updates remain explicit and low risk.
- New profiles still receive the documented initial centiscale ELO.
