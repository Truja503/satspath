/**
 * BIP-353 DNS Resolver — resolves `user@domain` via DNS TXT records
 * Uses DNS-over-HTTPS (Google/Cloudflare) for browser compatibility
 * Mirrors Rust bip353.rs resolve_bip353_with
 */

import { SignedPaymentProfile, ProfileResolver, AliasNotFoundError, TypedPaymentMethod, BitcoinNetwork } from "./types";

export type DnssecPolicy = "strict" | "dev_insecure";

export interface Bip353Resolution {
  bitcoin_uri: string;
  dnssec_valid: boolean;
  name: string;
}

interface DohResponse {
  Status: number;
  Answer?: Array<{
    name: string;
    type: number;
    TTL: number;
    data: string;
  }>;
  AD?: boolean;
}

const TXT_RECORD_TYPE = 16;
const DEFAULT_DOH_PROVIDERS = [
  "https://dns.google/resolve",
  "https://cloudflare-dns.com/dns-query",
];

export class Bip353Resolver implements ProfileResolver {
  private dohProviders: string[];
  private policy: DnssecPolicy;
  private cache: Map<string, { profile: SignedPaymentProfile; expires: number }> = new Map();
  private cacheTtl = 300_000; // 5 min

  constructor(dohProviders?: string[], policy: DnssecPolicy = "strict") {
    this.dohProviders = dohProviders || DEFAULT_DOH_PROVIDERS;
    this.policy = policy;
  }

  async resolve_alias(alias: string): Promise<SignedPaymentProfile> {
    const cached = this.cache.get(alias);
    if (cached && Date.now() < cached.expires) {
      return cached.profile;
    }

    const { name, domain } = this.parseAlias(alias);
    const txtName = `_bitcoin.payment.${domain}`;

    let resolution: Bip353Resolution | null = null;
    let lastError: Error | null = null;

    for (const provider of this.dohProviders) {
      try {
        resolution = await this.queryDoh(provider, txtName);
        break;
      } catch (e) {
        lastError = e as Error;
        continue;
      }
    }

    if (!resolution) {
      throw new AliasNotFoundError(`BIP-353 resolution failed for ${alias}: ${lastError?.message}`);
    }

    const profile = this.uriToProfile(alias, resolution.bitcoin_uri);
    
    this.cache.set(alias, { profile, expires: Date.now() + this.cacheTtl });
    return profile;
  }

  private parseAlias(alias: string): { name: string; domain: string } {
    const parts = alias.split("@");
    if (parts.length !== 2) throw new AliasNotFoundError(`Invalid alias format: ${alias}`);
    return { name: parts[0], domain: parts[1] };
  }

  private async queryDoh(provider: string, txtName: string): Promise<Bip353Resolution> {
    const url = `${provider}?name=${encodeURIComponent(txtName)}&type=${TXT_RECORD_TYPE}&do=1`;
    const headers: Record<string, string> = { accept: "application/dns-json" };
    
    if (provider.includes("cloudflare")) {
      headers.accept = "application/dns-json";
    }

    const response = await fetch(url, { headers });
    if (!response.ok) throw new Error(`DoH HTTP ${response.status}`);

    const data: DohResponse = await response.json();
    
    if (data.Status !== 0) throw new Error(`DNS error: ${data.Status}`);
    if (!data.Answer?.length) throw new Error("No TXT records found");

    if (this.policy === "strict" && !data.AD) {
      throw new Error("DNSSEC validation failed (AD flag not set)");
    }

    for (const record of data.Answer) {
      const txtData = record.data.replace(/^"|"$/g, "").replace(/\\"/g, '"');
      if (txtData.startsWith("bitcoin:") || txtData.startsWith("BIP321:")) {
        return {
          bitcoin_uri: txtData,
          dnssec_valid: data.AD === true,
          name: txtName,
        };
      }
    }

    throw new Error("No bitcoin: URI in TXT records");
  }

  private uriToProfile(alias: string, uri: string): SignedPaymentProfile {
    const methods: TypedPaymentMethod[] = [];
    
    try {
      // Parse BIP-321 URI: bitcoin:address?params...
      const cleanUri = uri.replace("BIP321:", "bitcoin:");
      const url = new URL(cleanUri.replace("bitcoin:", "https://") + "?");
      const address = url.pathname;
      
      if (address && this.isValidAddress(address)) {
        methods.push({
          type: "Onchain",
          label: "Bitcoin (BIP-353)",
          network: this.detectNetwork(address),
          address,
          address_list: [],
        });
      }

      const lightning = url.searchParams.get("lightning");
      if (lightning) {
        methods.push({
          type: "Lightning",
          label: "Lightning (BIP-353)",
          lightning_address: lightning,
        });
      }

      const lnurl = url.searchParams.get("lnurl");
      if (lnurl) {
        methods.push({
          type: "Lightning",
          label: "LNURL (BIP-353)",
          lnurl,
        });
      }

      const bolt12 = url.searchParams.get("bolt12");
      if (bolt12) {
        methods.push({
          type: "Lightning",
          label: "BOLT12 (BIP-353)",
          bolt12,
        });
      }

      const arkServer = url.searchParams.get("ark_server");
      const arkPubkey = url.searchParams.get("ark_pubkey");
      if (arkServer && arkPubkey) {
        methods.push({
          type: "Ark",
          label: "Ark (BIP-353)",
          server: arkServer,
          pubkey: arkPubkey,
        });
      }

    } catch {
      // Fallback: treat as on-chain only
      const addr = uri.replace("bitcoin:", "").split("?")[0];
      if (this.isValidAddress(addr)) {
        methods.push({
          type: "Onchain",
          label: "Bitcoin (BIP-353)",
          network: this.detectNetwork(addr),
          address: addr,
          address_list: [],
        });
      }
    }

    if (methods.length === 0) {
      throw new Error("No valid payment methods in BIP-353 URI");
    }

    // Create synthetic profile - signature verification happens downstream
    return {
      profile: {
        alias,
        identity_pubkey: this.domainPubkeyHash(alias.split("@")[1]),
        methods,
        updated_at: Math.floor(Date.now() / 1000),
        expires_at: Math.floor(Date.now() / 1000) + 86400, // 24h
        preferences: ["lightning", "onchain", "ark"],
        method_verifications: [],
      },
      signature: "", // Empty - caller must verify via DNSSEC or fetch signed profile
    };
  }

  private isValidAddress(addr: string): boolean {
    return /^(bc1|tb1|bcrt1|[13mn2])[a-zA-HJ-NP-Z0-9]{25,62}$/.test(addr);
  }

  private detectNetwork(addr: string): BitcoinNetwork {
    if (addr.startsWith("bc1") || addr.startsWith("1") || addr.startsWith("3")) return "mainnet";
    if (addr.startsWith("tb1") || addr.startsWith("m") || addr.startsWith("n") || addr.startsWith("2")) return "testnet";
    if (addr.startsWith("bcrt1")) return "regtest";
    return "mainnet";
  }

  private domainPubkeyHash(domain: string): string {
    // Deterministic pubkey from domain for DNSSEC-validated profiles
    let hash = 0;
    for (let i = 0; i < domain.length; i++) {
      hash = ((hash << 5) - hash) + domain.charCodeAt(i);
      hash |= 0;
    }
    const bytes = new Uint8Array(33);
    bytes[0] = 0x02;
    for (let i = 0; i < 32; i++) {
      bytes[i + 1] = (hash >> (i % 8)) & 0xff;
    }
    return Array.from(bytes).map(b => b.toString(16).padStart(2, "0")).join("");
  }

  clearCache(): void {
    this.cache.clear();
  }
}