import { describe, it, expect } from 'vitest';
import { ENVIRONMENTS, envById, isInsecureRemoteUrl, VPS_HOST } from './environments';

describe('envById', () => {
  it('returns the local environment config', () => {
    expect(envById('local')).toBe(ENVIRONMENTS.local);
    expect(envById('local').isProduction).toBe(false);
  });

  it('returns the production environment config, tunneled through loopback', () => {
    const env = envById('production');
    expect(env.isProduction).toBe(true);
    // The panel must never talk to the VPS directly — only through the local
    // end of the SSH tunnel (see environments.ts's module doc).
    expect(new URL(env.backendUrl).hostname).toBe('127.0.0.1');
    expect(env.tunnel?.sshHost).toBe(VPS_HOST);
  });
});

describe('isInsecureRemoteUrl', () => {
  it('accepts http:// on loopback', () => {
    expect(isInsecureRemoteUrl('http://127.0.0.1:8090')).toBe(false);
    expect(isInsecureRemoteUrl('http://localhost:8090')).toBe(false);
  });

  it('rejects http:// on any non-loopback host', () => {
    expect(isInsecureRemoteUrl('http://178.104.55.19:8090')).toBe(true);
    expect(isInsecureRemoteUrl('http://example.com')).toBe(true);
  });

  it('accepts https:// regardless of host', () => {
    expect(isInsecureRemoteUrl('https://178.104.55.19:8090')).toBe(false);
  });

  it('treats an unparseable URL as insecure', () => {
    expect(isInsecureRemoteUrl('not a url')).toBe(true);
  });
});
