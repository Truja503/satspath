/**
 * Nostr NIP-05 Resolver — discovers SatsPath profile via Nostr
 * 1. Fetch NIP-05 from `https://domain/.well-known/nostr.json?name=user`
 * 2. Query relays for kind 30078 with `d = satspath-profile:user@domain`
 * 3. Verify event author matches NIP-05 pubkey
 * Mirrors Rust NostrResolver
 */

import { SignedPaymentProfile, ProfileResolver, AliasNotFoundError, TypedPaymentMethod } from "./types";

interface Nip05Response {
  names: Record<string, string>; // user -> pubkey (hex)
  relays?: Record<string, string[]>; // pubkey -> relay URLs
}

interface NostrEvent {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
}

const SATSPATH_PROFILE_KIND = 30078;
const DEFAULT_RELAYS = [
  "wss://relay.nostr.band",
  "wss://nostr-pub.wellorder.net",
  "wss://relay.damus.io",
  "wss://relay.nostr.info",
];

export class NostrNip05Resolver implements ProfileResolver {
  private nip05Cache: Map<string, { pubkey: string; relays: string[]; expires: number }> = new Map();
  private profileCache: Map<string, { profile: SignedPaymentProfile; expires: number }> = new Map();
  private cacheTtl = 300_000; // 5 min
  private requestTimeout = 15_000;
  private customRelays?: string[];

  constructor(customRelays?: string[]) {
    this.customRelays = customRelays;
  }

  async resolve_alias(alias: string): Promise<SignedPaymentProfile> {
    const cached = this.profileCache.get(alias);
    if (cached && Date.now() < cached.expires) {
      return cached.profile;
    }

    const { name, domain } = this.parseAlias(alias);
    const dTag = `satspath-profile:${alias.toLowerCase()}`;

    // 1. Fetch NIP-05
    const nip05 = await this.fetchNip05(name, domain);
    const pubkey = nip05.names[name.toLowerCase()];
    if (!pubkey) throw new AliasNotFoundError(`NIP-05 not found for ${alias}`);

    // 2. Determine relays
    const relays = nip05.relays?.[pubkey] || this.customRelays || DEFAULT_RELAYS;

    // 3. Query relays for kind 30078 with matching d-tag
    const event = await this.queryRelaysForProfile(pubkey, dTag, relays);
    if (!event) throw new AliasNotFoundError(`SatsPath profile event not found for ${alias}`);

    // 4. Verify event author matches NIP-05 pubkey
    if (event.pubkey.toLowerCase() !== pubkey.toLowerCase()) {
      throw new Error(`Event author (${event.pubkey}) does not match NIP-05 pubkey (${pubkey})`);
    }

    // 5. Parse profile from event content
    let profileData: { profile: SignedPaymentProfile["profile"]; signature: string };
    try {
      profileData = JSON.parse(event.content);
    } catch {
      throw new Error("Invalid profile JSON in Nostr event content");
    }

    // 6. Validate
    const profile = this.validateProfile(alias, profileData);
    
    this.profileCache.set(alias, { profile, expires: Date.now() + this.cacheTtl });
    return profile;
  }

  private parseAlias(alias: string): { name: string; domain: string } {
    const parts = alias.split("@");
    if (parts.length !== 2) throw new AliasNotFoundError(`Invalid alias format: ${alias}`);
    return { name: parts[0], domain: parts[1] };
  }

  private async fetchNip05(name: string, domain: string): Promise<Nip05Response> {
    const cacheKey = `${name}@${domain}`;
    const cached = this.nip05Cache.get(cacheKey);
    if (cached && Date.now() < cached.expires) {
      return { names: { [name]: cached.pubkey }, relays: { [cached.pubkey]: cached.relays } };
    }

    const url = `https://${domain}/.well-known/nostr.json?name=${encodeURIComponent(name)}`;
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 10_000);

    try {
      const response = await fetch(url, {
        signal: controller.signal,
        headers: { accept: "application/json" },
      });
      clearTimeout(timeoutId);

      if (!response.ok) {
        if (response.status === 404) throw new AliasNotFoundError(`NIP-05 not found for ${name}@${domain}`);
        throw new Error(`NIP-05 HTTP ${response.status}`);
      }

      const data: Nip05Response = await response.json();
      
      if (data.names[name.toLowerCase()]) {
        this.nip05Cache.set(cacheKey, {
          pubkey: data.names[name.toLowerCase()],
          relays: data.relays?.[data.names[name.toLowerCase()]] || [],
          expires: Date.now() + this.cacheTtl,
        });
      }

      return data;

    } catch (e) {
      clearTimeout(timeoutId);
      if (e instanceof AliasNotFoundError) throw e;
      throw new Error(`NIP-05 fetch failed for ${name}@${domain}: ${(e as Error).message}`);
    }
  }

  private async queryRelaysForProfile(
    pubkey: string,
    dTag: string,
    relays: string[]
  ): Promise<NostrEvent | null> {
    // Try relays in parallel, return first valid result
    const promises = relays.map(relay => this.querySingleRelay(relay, pubkey, dTag));
    
    for (const promise of promises) {
      try {
        const event = await promise;
        if (event) return event;
      } catch {
        // Try next relay
      }
    }
    return null;
  }

  private querySingleRelay(relay: string, pubkey: string, dTag: string): Promise<NostrEvent | null> {
    return new Promise((resolve, reject) => {
      let ws: WebSocket;
      let resolved = false;
      const timeout = setTimeout(() => {
        if (!resolved) {
          resolved = true;
          ws?.close();
          reject(new Error(`Relay timeout: ${relay}`));
        }
      }, this.requestTimeout);

      try {
        ws = new WebSocket(relay);
      } catch (e) {
        clearTimeout(timeout);
        reject(new Error(`WebSocket connection failed: ${relay}`));
        return;
      }

      ws.onopen = () => {
        if (resolved) { ws.close(); return; }
        const req = ["REQ", `satspath-${Date.now()}`, {
          kinds: [SATSPATH_PROFILE_KIND],
          authors: [pubkey],
          "#d": [dTag],
          limit: 1,
        }];
        ws.send(JSON.stringify(req));
      };

      ws.onmessage = (msgEvent) => {
        if (resolved) return;
        try {
          const msg = JSON.parse(msgEvent.data);
          if (msg[0] === "EVENT") {
            const event: NostrEvent = msg[2];
            const dTagMatch = event.tags.find(t => t[0] === "d" && t[1] === dTag);
            if (dTagMatch) {
              resolved = true;
              clearTimeout(timeout);
              ws.close();
              resolve(event);
            }
          } else if (msg[0] === "EOSE") {
            // End of stored events
          }
        } catch {
          // Ignore parse errors
        }
      };

      ws.onerror = (err) => {
        if (!resolved) {
          resolved = true;
          clearTimeout(timeout);
          reject(new Error(`Relay error: ${relay}`));
        }
      };

      ws.onclose = () => {
        if (!resolved) {
          clearTimeout(timeout);
          reject(new Error(`Relay closed: ${relay}`));
        }
      };
    });
  }

  private validateProfile(alias: string, data: { profile: SignedPaymentProfile["profile"]; signature: string }): SignedPaymentProfile {
    if (!data.profile || !data.signature) {
      throw new Error("Invalid profile data: missing profile or signature");
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
        break;
      case "Lightning":
        if (!method.lightning_address && !method.lnurl && !method.bolt12) {
          throw new Error("Lightning method missing all pointers");
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

  clearCaches(): void {
    this.nip05Cache.clear();
    this.profileCache.clear();
  }
}