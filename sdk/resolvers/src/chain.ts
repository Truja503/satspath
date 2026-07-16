/**
 * Resolver Chain — combines multiple resolvers in priority order
 * Mirrors Rust ChainResolver
 */

import { SignedPaymentProfile, ProfileResolver, AliasNotFoundError } from "./types";
import { LocalRegistryResolver } from "./localRegistry";
import { Bip353Resolver } from "./bip353";
import { HttpWellKnownResolver } from "./httpWellKnown";
import { NostrNip05Resolver } from "./nostrNip05";

export class ChainResolver implements ProfileResolver {
  private resolvers: ProfileResolver[] = [];

  constructor(resolvers?: ProfileResolver[]) {
    if (resolvers) {
      this.resolvers = resolvers;
    } else {
      // Default chain: local → BIP-353 → HTTPS well-known → Nostr NIP-05
      this.resolvers = [
        new LocalRegistryResolver(),
        new Bip353Resolver(),
        new HttpWellKnownResolver(),
        new NostrNip05Resolver(),
      ];
    }
  }

  push(resolver: ProfileResolver): ChainResolver {
    this.resolvers.push(resolver);
    return this;
  }

  prepend(resolver: ProfileResolver): ChainResolver {
    this.resolvers.unshift(resolver);
    return this;
  }

  async resolve_alias(alias: string): Promise<SignedPaymentProfile> {
    let lastError: Error | null = null;

    for (const resolver of this.resolvers) {
      try {
        return await resolver.resolve_alias(alias);
      } catch (e) {
        lastError = e as Error;
        // Continue to next resolver on AliasNotFoundError or network errors
        if (!(e instanceof AliasNotFoundError)) {
          console.warn(`Resolver ${resolver.constructor.name} failed for ${alias}:`, e);
        }
      }
    }

    throw lastError || new AliasNotFoundError(`No resolver found profile for ${alias}`);
  }

  getResolvers(): ProfileResolver[] {
    return [...this.resolvers];
  }

  clearCaches(): void {
    for (const resolver of this.resolvers) {
      if ("clearCache" in resolver && typeof resolver.clearCache === "function") {
        (resolver as { clearCache: () => void }).clearCache();
      }
    }
  }
}

// Default singleton
let defaultChain: ChainResolver | null = null;

export function getDefaultChain(): ChainResolver {
  if (!defaultChain) {
    defaultChain = new ChainResolver();
  }
  return defaultChain;
}

export function setDefaultChain(chain: ChainResolver): void {
  defaultChain = chain;
}