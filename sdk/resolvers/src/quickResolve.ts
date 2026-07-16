/**
 * Quick resolve functions using default chain
 * Separated to avoid circular imports
 */

import { ChainResolver } from "./chainResolver";
import { SignedPaymentProfile, AliasNotFoundError } from "./types";

let _defaultChain: ChainResolver | null = null;

export async function getDefaultChain(): Promise<ChainResolver> {
  if (!_defaultChain) {
    _defaultChain = await createDefaultChainInternal();
  }
  return _defaultChain;
}

async function createDefaultChainInternal(): Promise<ChainResolver> {
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

export async function resolveAlias(alias: string): Promise<SignedPaymentProfile> {
  const chain = await getDefaultChain();
  return chain.resolve_alias(alias);
}

export async function verifyProfile(_profile: SignedPaymentProfile): Promise<boolean> {
  // Profile signature verification would use WASM crypto
  // For now, return true - WASM integration happens in router package
  return true;
}