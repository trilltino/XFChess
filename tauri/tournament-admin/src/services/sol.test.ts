import { describe, it, expect } from 'vitest';
import {
  LAMPORTS_PER_SOL,
  solInputToLamports,
  lamportsToSolInput,
  lamportsToUsd,
  usdInputToLamports,
  lamportsToUsdInput,
} from './sol';

describe('solInputToLamports', () => {
  it('converts a whole-SOL input', () => {
    expect(solInputToLamports('1')).toBe(LAMPORTS_PER_SOL);
    expect(solInputToLamports('0.5')).toBe(LAMPORTS_PER_SOL / 2);
  });

  it('rounds fractional lamports', () => {
    expect(solInputToLamports('0.000000001234')).toBe(1); // rounds to 1 lamport
  });

  it('treats invalid input as 0', () => {
    expect(solInputToLamports('not a number')).toBe(0);
    expect(solInputToLamports('')).toBe(0);
  });

  it('treats negative input as 0', () => {
    expect(solInputToLamports('-1')).toBe(0);
  });
});

describe('lamportsToSolInput', () => {
  it('round-trips solInputToLamports for whole SOL amounts', () => {
    expect(lamportsToSolInput(LAMPORTS_PER_SOL)).toBe('1');
    expect(lamportsToSolInput(LAMPORTS_PER_SOL * 2.5)).toBe('2.5');
  });
});

describe('lamportsToUsd / usdInputToLamports', () => {
  it('lamportsToUsd is null with no loaded rate', () => {
    expect(lamportsToUsd(LAMPORTS_PER_SOL, null)).toBeNull();
  });

  it('lamportsToUsd converts using the given rate', () => {
    expect(lamportsToUsd(LAMPORTS_PER_SOL, 150)).toBe(150);
    expect(lamportsToUsd(LAMPORTS_PER_SOL / 2, 150)).toBe(75);
  });

  it('usdInputToLamports converts back using the same rate', () => {
    expect(usdInputToLamports('150', 150)).toBe(LAMPORTS_PER_SOL);
  });

  it('usdInputToLamports is 0 when no rate is loaded', () => {
    expect(usdInputToLamports('150', null)).toBe(0);
  });

  it('usdInputToLamports treats invalid/negative input as 0', () => {
    expect(usdInputToLamports('not a number', 150)).toBe(0);
    expect(usdInputToLamports('-5', 150)).toBe(0);
  });
});

describe('lamportsToUsdInput', () => {
  it('formats to 2 decimal places', () => {
    expect(lamportsToUsdInput(LAMPORTS_PER_SOL, 149.999)).toBe('150.00');
  });

  it('is an empty string with no loaded rate', () => {
    expect(lamportsToUsdInput(LAMPORTS_PER_SOL, null)).toBe('');
  });
});
