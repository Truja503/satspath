/**
 * Chain Resolver — combines multiple resolvers in priority order
 * Mirrors Rust ChainResolver exactly
 */

import { ProfileResolver, SignedPaymentProfile, AliasNotFoundError } from "./types";

export class ChainResolver implements ProfileResolver {
  private resolvers: ProfileResolver[] = [];

  push(resolver: ProfileResolver): this {
    this.resolvers.push(resolver);
    return this;
  }

  async resolve_alias(alias: string): Promise<SignedPaymentProfile> {
    let lastError: Error | null = null;
    
    for (const resolver of this.resolvers) {
      try {
        return await resolver.resolve_alias(alias);
      } catch (e) {
        lastError = e as Error;
        
        // If alias not found, continue to next resolver
        if (e instanceof AliasNotFoundError) {
          continue;
        }
        
        // For other errors, log but continue
        console.warn(`Resolver ${resolver.constructor.name} failed:`, e);
      }
    }
    
    // All resolvers exhausted
    throw lastError || new AliasNotFoundError(alias);
  }
}

/** Create default resolver chain - async to allow dynamic imports */
export async function createDefaultChain(): Promise<ChainResolver> {
  const { LocalRegistryResolver } = await import("./localRegistry");
  const { Bip353Resolver } = await import("./bip353");
  const { HttpWellKnownResolver } = await import("./httpWellKnown");
  const { NostrNip05Resolver } = await import("./nostrNip05");

  const chain = new ChainResolver();
  
  // 1. Local registry (in-memory + localStorage)
  chain.push(new LocalRegistryResolver());
  
  // 2. BIP-353 DNS
  chain.push(new Bip353Resolver());
  
  // 3. HTTPS Well-Known
  chain.push(new HttpWellKnownResolver());
  
  // 4. Nostr NIP-05
  chain.push(new NostrNip05Resolver());
  
  return chain;
}

/** Create chain with custom resolvers */
export function createCustomChain(resolvers: ProfileResolver[]): ChainResolver {
  const chain = new ChainResolver();
  for (const resolver of resolvers) {
    chain.push(resolver);
  }
  return chain;
}