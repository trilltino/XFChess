/**
 * KYC and user verification-status endpoints.
 *
 * `submitKyc` posts the identity payload to the backend vault;
 * `getUserStatus` reports which gates (profile / email / KYC) the user
 * has completed so the UI can show what still needs doing before
 * wagered play.
 */

import { request } from './client';

export interface KycSubmission {
  wallet_pubkey: string;
  country: string;
  full_name: string;
  dob: string; // YYYY-MM-DD
  residence: string;
  tax_id: string;
}

export interface UserStatus {
  has_profile: boolean;
  has_email: boolean;
  has_kyc: boolean;
  can_wager: boolean;
}

/** Submit KYC data to the backend identity vault. */
export function submitKyc(body: KycSubmission): Promise<{ ok: boolean }> {
  return request('/api/kyc/submit', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/** Get the verification status for a wallet pubkey. */
export function getUserStatus(pubkey: string): Promise<UserStatus> {
  return request(`/api/user/status/${pubkey}`, { method: 'GET' });
}
