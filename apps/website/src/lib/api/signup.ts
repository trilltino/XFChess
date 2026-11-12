/** Backend email sign-up: POST /api/signup queues a welcome email (sent through Resend). */

import { BACKEND_URL } from './client';

export interface SignupRequest {
  email: string;
  /** Shown in the welcome email; the backend falls back to the part of the address before @. */
  username?: string;
}

/** A sign-up failure with a message that is safe to show the visitor. */
export class SignupError extends Error {}

/**
 * Sign up for XFChess. Resolves when the backend accepted the address (the welcome email is
 * queued, not necessarily delivered yet); otherwise throws a SignupError.
 *
 * Does not use `request()`: nginx answers an over-limit client with an HTML error page, which
 * must never be shown to the visitor.
 */
export async function signUp(body: SignupRequest): Promise<void> {
  let res: Response;
  try {
    res = await fetch(`${BACKEND_URL}/api/signup`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        email: body.email.trim(),
        ...(body.username?.trim() ? { username: body.username.trim() } : {}),
      }),
    });
  } catch {
    throw new SignupError('Could not reach XFChess. Check your connection and try again.');
  }
  if (res.ok) return;
  if (res.status === 400) throw new SignupError('Please enter a valid email address.');
  if (res.status === 429 || res.status === 503) {
    throw new SignupError('Too many attempts. Please wait a minute and try again.');
  }
  throw new SignupError('Something went wrong on our side. Please try again later.');
}
