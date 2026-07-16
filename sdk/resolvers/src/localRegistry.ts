/**
 * Local Registry Resolver — in-memory + localStorage for PWA
 * Mirrors Rust Registry::open + resolve_alias
 */

import { SignedPaymentProfile, ProfileResolver, AliasNotFoundError } from "./types";

const STORAGE_KEY = "satspath:registry:v1";

export interface LocalRegistryOptions {
  initialProfiles?: SignedPaymentProfile[];
  storage?: Storage;
}

export class LocalRegistryResolver implements ProfileResolver {
  private registry: Map<string, SignedPaymentProfile>;
  private storage: Storage;

  constructor(options: LocalRegistryOptions = {}) {
    this.storage = options.storage || (typeof localStorage !== "undefined" ? localStorage : createMemoryStorage());
    this.registry = new Map();
    
    this.loadFromStorage();
    
    if (options.initialProfiles) {
      for (const profile of options.initialProfiles) {
        this.registry.set(this.canonicalAlias(profile.profile.alias), profile);
      }
    }
  }

  private loadFromStorage(): void {
    try {
      const stored = this.storage.getItem(STORAGE_KEY);
      if (stored) {
        const profiles: SignedPaymentProfile[] = JSON.parse(stored);
        for (const profile of profiles) {
          this.registry.set(this.canonicalAlias(profile.profile.alias), profile);
        }
      }
    } catch (e) {
      console.warn("Failed to load local registry:", e);
    }
  }

  private saveToStorage(): void {
    try {
      const profiles = Array.from(this.registry.values());
      this.storage.setItem(STORAGE_KEY, JSON.stringify(profiles));
    } catch (e) {
      console.warn("Failed to save local registry:", e);
    }
  }

  private canonicalAlias(alias: string): string {
    return alias.toLowerCase().trim();
  }

  async resolve_alias(alias: string): Promise<SignedPaymentProfile> {
    const profile = this.registry.get(this.canonicalAlias(alias));
    if (!profile) throw new AliasNotFoundError(alias);
    return profile;
  }

  // Management methods
  addProfile(profile: SignedPaymentProfile): void {
    this.registry.set(this.canonicalAlias(profile.profile.alias), profile);
    this.saveToStorage();
  }

  removeProfile(alias: string): boolean {
    const result = this.registry.delete(this.canonicalAlias(alias));
    if (result) this.saveToStorage();
    return result;
  }

  listProfiles(): SignedPaymentProfile[] {
    return Array.from(this.registry.values());
  }

  clear(): void {
    this.registry.clear();
    this.saveToStorage();
  }

  static fromRustRegistry(json: string): LocalRegistryResolver {
    const data = JSON.parse(json);
    const profiles: SignedPaymentProfile[] = data.profiles || [];
    return new LocalRegistryResolver({ initialProfiles: profiles });
  }
}

function createMemoryStorage(): Storage {
  const store = new Map<string, string>();
  return {
    getItem: (key: string) => store.get(key) || null,
    setItem: (key: string, value: string) => store.set(key, value),
    removeItem: (key: string) => store.delete(key),
    clear: () => store.clear(),
    key: (index: number) => Array.from(store.keys())[index] || null,
    get length() { return store.size; },
  };
}

// Singleton
let defaultRegistry: LocalRegistryResolver | null = null;

export function getDefaultRegistry(): LocalRegistryResolver {
  if (!defaultRegistry) {
    defaultRegistry = new LocalRegistryResolver();
  }
  return defaultRegistry;
}

export function setDefaultRegistry(registry: LocalRegistryResolver): void {
  defaultRegistry = registry;
}