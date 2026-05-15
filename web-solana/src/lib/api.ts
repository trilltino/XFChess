/**
 * Thin HTTP helpers for the XFChess backend.
 *
 * This file is a facade that re-exports the feature-grouped modules under
 * `lib/api/*`. All existing call sites (`import { foo } from '../lib/api'`)
 * continue to work unchanged.
 *
 * Submodules:
 * - `./api/client`     — shared `request()` helper and `BACKEND_URL`
 * - `./api/auth`       — signup, wallet/email login, username & profile sync
 * - `./api/kyc`        — KYC submission and user verification status
 * - `./api/games`      — game history and dispute endpoints
 * - `./api/tournament` — Swiss tournament state, pairings, standings, results
 */

export * from './api/client';
export * from './api/auth';
export * from './api/kyc';
export * from './api/games';
export * from './api/tournament';
