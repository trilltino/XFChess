import { beforeEach, describe, expect, it } from 'vitest';
import { envById, getOpsConfig, isInsecureRemoteUrl, opsConfigProblem, saveOpsConfig } from './environments';

const store = new Map<string, string>();
beforeEach(() => {
  store.clear();
  (globalThis as any).localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  };
});

describe('envById', () => {
  it('local talks to the local backend without a tunnel', () => {
    const env = envById('local');
    expect(env.isProduction).toBe(false);
    expect(env.tunnel).toBeUndefined();
  });

  it('production tunnels to the configured host with the configured key', () => {
    const env = envById('production', { vpsHost: '10.0.0.5', sshKeyPath: '~/.ssh/ops' });
    expect(env.isProduction).toBe(true);
    expect(env.tunnel?.sshHost).toBe('10.0.0.5');
    expect(env.tunnel?.sshKey).toBe('~/.ssh/ops');
  });
});

describe('ops config', () => {
  it('saved values override the defaults', () => {
    saveOpsConfig({ vpsHost: ' 10.0.0.9 ', sshKeyPath: 'C:/keys/ops' });
    expect(getOpsConfig()).toEqual({ vpsHost: '10.0.0.9', sshKeyPath: 'C:/keys/ops' });
  });

  it('flags hosts and key paths the shell scope would reject', () => {
    expect(opsConfigProblem({ vpsHost: '', sshKeyPath: '~/.ssh/k' })).toMatch(/VPS host/);
    expect(opsConfigProblem({ vpsHost: '1.2.3.4', sshKeyPath: '-oProxyCommand=x' })).toMatch(/SSH key/);
    expect(opsConfigProblem({ vpsHost: '1.2.3.4', sshKeyPath: 'C:/Users/me/.ssh/k' })).toBeNull();
  });
});

describe('isInsecureRemoteUrl', () => {
  it('allows loopback http', () => {
    expect(isInsecureRemoteUrl('http://127.0.0.1:8090')).toBe(false);
    expect(isInsecureRemoteUrl('http://localhost:8090')).toBe(false);
  });

  it('rejects remote http', () => {
    expect(isInsecureRemoteUrl('http://178.104.55.19:8090')).toBe(true);
    expect(isInsecureRemoteUrl('http://example.com')).toBe(true);
  });

  it('allows https', () => {
    expect(isInsecureRemoteUrl('https://178.104.55.19:8090')).toBe(false);
  });

  it('rejects unparseable urls', () => {
    expect(isInsecureRemoteUrl('not a url')).toBe(true);
  });
});
