/**
 * HTTPS Well-Known Resolver — fetches signed profile from `/.well-known/satspath/<alias>.json`
 * Mirrors Rust HttpResolver
 */

import { SignedPaymentProfile, ProfileResolver, AliasNotFoundError, TypedPaymentMethod, BitcoinNetwork } from "./types";

export interface WellKnownProfile {
  profile: SignedPaymentProfile["profile"];
  signature: string;
}

export class HttpWellKnownResolver implements ProfileResolver {
  private baseUrl: string;
  private cache: Map<string, { profile: SignedPaymentProfile; expires: number }> = new Map();
  private cacheTtl = 300_000; // 5 min
  private timeout = 10_000;

  constructor(baseUrl?: string) {
    this.baseUrl = baseUrl || "";
  }

  async resolve_alias(alias: string): Promise<SignedPaymentProfile> {
    const cached = this.cache.get(alias);
    if (cached && Date.now() < cached.expires) {
      return cached.profile;
    }

    const { name, domain } = this.parseAlias(alias);
    const url = `${this.baseUrl}https://${domain}/.well-known/satspath/${encodeURIComponent(name)}.json`;

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(url, {
        signal: controller.signal,
        headers: { accept: "application/json" },
      });
      clearTimeout(timeoutId);

      if (!response.ok) {
        if (response.status === 404) throw new AliasNotFoundError(alias);
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data: WellKnownProfile = await response.json();
      const profile = this.validateProfile(alias, data);
      
      this.cache.set(alias, { profile, expires: Date.now() + this.cacheTtl });
      return profile;

    } catch (e) {
      clearTimeout(timeoutId);
      if (e instanceof AliasNotFoundError) throw e;
      if (e instanceof DOMException && e.name === "AbortError") {
        throw new Error(`Timeout fetching well-known profile for ${alias}`);
      }
      throw new Error(`Well-known fetch failed for ${alias}: ${(e as Error).message}`);
    }
  }

  private parseAlias(alias: string): { name: string; domain: string } {
    const parts = alias.split("@");
    if (parts.length !== 2) throw new AliasNotFoundError(`Invalid alias format: ${alias}`);
    return { name: parts[0], domain: parts[1] };
  }

  private validateProfile(alias: string, data: WellKnownProfile): SignedPaymentProfile {
    if (!data.profile || !data.signature) {
      throw new Error("Invalid well-known response: missing profile or signature");
    }

    if (data.profile.alias.toLowerCase() !== alias.toLowerCase()) {
      throw new Error(`Profile alias mismatch: expected ${alias}, got ${data.profile.alias}`);
    }

    for (const method of data.profile.methods) {
      this.validateMethod(method);
    }

    if (data.profile.expires_at && data.profile.expires_at * 1000 < Date.now()) {
      throw new Error("Profile expired");
    }

    return {
      profile: data.profile,
      signature: data.signature,
    };
  }

  private validateMethod(method: TypedPaymentMethod): void {
    switch (method.type) {
      case "Onchain":
        if (!method.address && !method.silent_payment_pubkey) {
          throw new Error("Onchain method missing address and silent_payment_pubkey");
        }
        if (method.address && !this.isValidAddress(method.address, method.network)) {
          throw new Error(`Invalid ${method.network} address: ${method.address}`);
        }
        break;
      case "Lightning":
        if (!method.lightning_address && !method.lnurl && !method.bolt12) {
          throw new Error("Lightning method missing lightning_address, lnurl, and bolt12");
        }
        break;
      case "Ark":
        if (!method.server || !method.pubkey) {
          throw new Error("Ark method missing server or pubkey");
        }
        if (!method.server.startsWith("https://")) {
          throw new Error("Ark server must be HTTPS");
        }
        break;
    }
  }

  private isValidAddress(addr: string, network: BitcoinNetwork): boolean {
    const prefixes: Record<BitcoinNetwork, string[]> = {
      mainnet: ["bc1", "1", "3"],
      testnet: ["tb1", "m", "n", "2"],
      regtest: ["bcrt1"],
    };
    return prefixes[network].some(p => addr.startsWith(p)) && addr.length >= 26 && addr.length <= 62;
  }

  clearCache(): void {
    this.cache.clear();
  }

  setTimeout(ms: number): void {
    this.timeout = ms;
  }
}