export interface StablecoinInfo {
  mint: string;
  symbol: string;
  name: string;
  decimals: number;
}

export const GLOBAL_STABLECOINS: StablecoinInfo[] = [
  {
    mint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    symbol: 'USDC',
    name: 'USD Coin',
    decimals: 6,
  },
  {
    mint: 'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
    symbol: 'USDT',
    name: 'Tether USD',
    decimals: 6,
  },
];

/** Per-country stablecoin recommendations. Falls back to USDC/USDT for unsupported jurisdictions. */
export const COUNTRY_STABLECOINS: Record<string, StablecoinInfo[]> = {
  GB: [GLOBAL_STABLECOINS[0]],
  US: [GLOBAL_STABLECOINS[0]],
  BR: [GLOBAL_STABLECOINS[0]],
  CA: [GLOBAL_STABLECOINS[0]],
  DE: [GLOBAL_STABLECOINS[0]],
  FR: [GLOBAL_STABLECOINS[0]],
  // TODO: Add verified EURC mint for EU countries once on-chain address is confirmed.
};

export function getStablecoinsForCountry(countryCode: string): StablecoinInfo[] {
  return COUNTRY_STABLECOINS[countryCode.toUpperCase()] ?? GLOBAL_STABLECOINS;
}

export function getCountryStablecoinMint(countryCode: string): string | undefined {
  return getStablecoinsForCountry(countryCode)[0]?.mint;
}
