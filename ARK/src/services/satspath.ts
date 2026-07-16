// Satspath service for Arkade wallet - integrates with satspath-wasm
// This service provides a clean TypeScript API over the WASM module

import { init, quote, resolve_alias, verify_signed_profile, build_qr_payload, ChainResolver, LocalRegistry, SignedPaymentProfile, PaymentProfile, PaymentMethod, PaymentMethodType, FeeEstimate, RouteQuote, ExecutionMode, QuoteResponse, QuoteRecipient, Invite, BitcoinNetwork, SwapDirective, PaymentUrgency } from '@satspath/wasm';

// WASM module types (these match the Rust types exactly)
export interface WasmPaymentMethod {
  type: 'onchain' | 'lightning' | 'ark';
  label: string;
  network?: BitcoinNetwork;
  address?: string;
  silent_payment_pubkey?: string;
  pubkey_hint?: string;
  descriptor_hint?: string;
  address_list?: string[];
  lightning_address?: string;
  lnurl?: string;
  bolt12?: string;
  receiver_pubkey?: string;
  server?: string;
  pubkey?: string;
  vtxo_pointer?: string;
  opaque_uri?: string;
  proof?: any;
  expires_at?: number;
}

export interface WasmPaymentProfile {
  alias: string;
  identity_pubkey: string;
  methods: WasmPaymentMethod[];
  updated_at: number;
  expires_at?: number;
  sequence?: number;
  preferences: string[];
  nonce?: string;
  rotation?: any;
  method_verifications: any[];
}

export interface WasmSignedPaymentProfile {
  profile: WasmPaymentProfile;
  signature: string;
}

export interface WasmQuoteResponse {
  status: 'ok' | 'not_registered' | 'no_route' | 'invalid_signature';
  recipient?: WasmQuoteRecipient;
  selected_method?: WasmPaymentMethod;
  fee_sats?: number;
  eta?: string;
  reason?: string;
  qr?: string;
  execution?: ExecutionMode;
  wallet_hint?: string;
  invite?: WasmInvite;
}

export interface WasmQuoteRecipient {
  alias: string;
  verified: boolean;
  profile_signature_verified: boolean;
  identifier_verified: boolean;
  identifier_verification: string;
  fingerprint?: string;
}

export interface WasmInvite {
  alias_hash: string;
  amount_sats: number;
  created_at: number;
  expires_at: number;
  claim_url: string;
  warning: string;
  sender_signature?: string;
  sender_pubkey?: string;
}

export interface WasmFeeEstimate {
  fastest_fee: number;
  half_hour_fee: number;
  hour_fee: number;
  economy_fee: number;
  minimum_fee: number;
}

export interface WasmRouteRequest {
  alias: string;
  amount_sats: number;
  signed_profile: WasmSignedPaymentProfile;
  urgency: 'low' | 'normal' | 'high';
  max_fee_sats?: number;
  max_fee_percent?: number;
}

export interface WasmRouteQuote {
  selected_method: WasmPaymentMethod;
  reason: string;
  estimated_fee_sats: number;
  estimated_confirmation: string;
  fee_snapshot?: WasmFeeEstimate;
  swap_directive: SwapDirective;
  execution: ExecutionMode;
  wallet_hint: string;
}

export interface WasmIdentity {
  pubkey: string;
  secret_key_path: string;
  fingerprint: string;
}

export interface SatspathConfig {
  wasmPath?: string;
  autoInit?: boolean;
}

class SatspathServiceClass {
  private initialized = false;
  private wasmModule: any = null;
  private config: SatspathConfig;

  constructor(config: SatspathConfig = {}) {
    this.config = config;
  }

  /**
   * Initialize the WASM module
   */
  async init(): Promise<void> {
    if (this.initialized) return;
    
    try {
      // Dynamic import of the WASM module
      // In production, this would be: import('@satspath/wasm');
      // For development, we assume the wasm is available at the configured path
      if (this.config.wasmPath) {
        const wasmModule = await import(this.config.wasmPath);
        this.wasmModule = wasmModule;
      } else {
        // Try to load from the default satspath-sdk location
        const wasmModule = await import('@satspath/wasm');
        this.wasmModule = wasmModule.default || wasmModule;
      }
      
      // Initialize the WASM module
      if (this.wasmModule.main) {
        this.wasmModule.main();
      }
      
      this.initialized = true;
    } catch (e) {
      console.error('Failed to initialize satspath-wasm:', e);
      throw new Error('Failed to load satspath-wasm module. Ensure it is built and available.');
    }
  }

  /**
   * Ensure WASM is initialized
   */
  private async ensureInit(): Promise<void> {
    if (!this.initialized) {
      await this.init();
    }
  }

  // ============================================================
  // Identity & Key Management
  // ============================================================

  /**
   * Generate a new SatsPath identity keypair
   */
  async generateIdentity(): Promise<{ pubkey: string; fingerprint: string }> {
    await this.ensureInit();
    
    // This would call the WASM function to generate a keypair
    // For now, we use a mock implementation
    const keyPair = await this.generateKeyPair();
    return {
      pubkey: keyPair.pubkey,
      fingerprint: this.fingerprintPubkey(keyPair.pubkey)
    };
  }

  /**
   * Get fingerprint of a pubkey (first 8 hex chars after 02/03 prefix)
   */
  fingerprintPubkey(pubkeyHex: string): string {
    if (!pubkeyHex || pubkeyHex.length !== 66) return '';
    return pubkeyHex.slice(2, 10); // Skip 02/03 prefix, take 8 chars
  }

  /**
   * Mask an identifier for display (e.g., alice@example.com -> a***e@example.com)
   */
  maskIdentifier(alias: string): string {
    const parts = alias.split('@');
    if (parts.length !== 2) return '***';
    const local = parts[0];
    const domain = parts[1];
    if (local.length <= 2) return `***@${domain}`;
    return `${local[0]}***${local[local.length - 1]}@${domain}`;
  }

  // ============================================================
  // Profile Resolution & Verification
  // ============================================================

  /**
   * Resolve an alias to a signed payment profile
   */
  async resolveAlias(alias: string): Promise<WasmSignedPaymentProfile> {
    await this.ensureInit();
    
    const chainResolver = this.wasmModule.ChainResolver?.new();
    if (!chainResolver) {
      throw new Error('ChainResolver not available in WASM module');
    }
    
    const profileJson = await chainResolver.resolve_alias(alias);
    return JSON.parse(profileJson);
  }

  /**
   * Verify a signed payment profile's signature
   */
  async verifyProfile(profile: WasmSignedPaymentProfile): Promise<boolean> {
    await this.ensureInit();
    
    const profileJson = JSON.stringify(profile);
    return this.wasmModule.verify_signed_profile(profileJson);
  }

  /**
   * Sign a payment profile with an identity key
   * In production, this would use the private key from secure storage
   */
  async signProfile(profile: WasmPaymentProfile, pubkeyHex: string): Promise<WasmSignedPaymentProfile> {
    // This is a mock - real implementation would sign with the private key
    // The WASM module can verify but signing requires the private key
    // which should be kept in secure storage (Secure Enclave / Keystore)
    const profileJson = JSON.stringify({
      ...profile,
      nonce: Math.random().toString(36).substring(2, 15),
      updated_at: Math.floor(Date.now() / 1000)
    });
    
    // Generate a mock signature (in production, use secp256k1 signing)
    const mockSignature = '00'.repeat(64); // 64 bytes hex
    
    return {
      profile: {
        ...profile,
        nonce: Math.random().toString(36).substring(2, 15),
        updated_at: Math.floor(Date.now() / 1000)
      },
      signature: mockSignature
    };
  }

  // ============================================================
  // Routing & Quotes
  // ============================================================

  /**
   * Get a payment quote for a recipient and amount
   * This resolves, verifies, routes, and builds the payment payload
   */
  async getQuote(recipient: string, amountSats: number): Promise<WasmQuoteResponse> {
    await this.ensureInit();
    
    // Use the high-level quote function from WASM
    const quoteJson = await this.wasmModule.quote(recipient, amountSats);
    return JSON.parse(quoteJson);
  }

  /**
   * Build a QR payment payload for a method
   */
  buildQrPayload(method: any, amountSats: number): string {
    // This would call the WASM function
    // For now, implement in TypeScript
    switch (method.type) {
      case 'lightning':
        return method.lnurl || method.lightning_address || method.bolt12 || '';
      case 'onchain': {
        const target = method.silent_payment_pubkey || method.address || '';
        const btc = (amountSats / 100_000_000).toFixed(8);
        return `bitcoin:${target}?amount=${btc}`;
      }
      case 'ark':
        return `ark:${encodeURIComponent(method.pubkey)}?server=${encodeURIComponent(method.server || '')}&amount=${amountSats}`;
      default:
        return '';
    }
  }

  // ============================================================
  // Identity & Profile Management
  // ============================================================

  /**
   * Get the default resolver chain
   */
  getDefaultResolver() {
    return this.wasmModule.ChainResolver?.new();
  }

  /**
   * Get the local registry for managing local profiles
   */
  getLocalRegistry() {
    return this.wasmModule.LocalRegistry?.new();
  }

  // ============================================================
  // Helper Functions
  // ============================================================

  /**
   * Mock key pair generation (replace with real secp256k1 in production)
   */
  private async generateKeyPair(): Promise<{ pubkey: string; privkey: string }> {
    // This is a mock - in production use secp256k1 with proper RNG
    const array = new Uint8Array(32);
    crypto.getRandomValues(array);
    const privkey = Array.from(array).map(b => b.toString(16).padStart(2, '0')).join('');
    
    // Derive pubkey (mock - real implementation uses secp256k1)
    const pubkey = '02' + '00'.repeat(32);
    
    return { pubkey, privkey };
  }

  /**
   * Create an invite for an unregistered user
   */
  async createInvite(alias: string, amountSats: number): Promise<WasmInvite> {
    await this.ensureInit();
    
    // This would call the WASM function
    // For now, return a mock invite
    const aliasHash = this.simpleHash(alias);
    return {
      alias_hash: aliasHash,
      amount_sats: amountSats,
      created_at: Math.floor(Date.now() / 1000),
      expires_at: Math.floor(Date.now() / 1000) + 24 * 60 * 60,
      claim_url: `https://satspath.local/claim?alias_hash=${aliasHash.slice(0, 16)}&amount=${amountSats}`,
      warning: 'The receiver must claim this payment by generating their own keys locally. SatsPath never holds or generates keys on behalf of users.'
    };
  }

  /**
   * Simple hash function (replace with SHA-256 in production)
   */
  private simpleHash(str: string): string {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      hash = ((hash << 5) - hash) + str.charCodeAt(i);
      hash |= 0;
    }
    return Math.abs(hash).toString(16).padStart(64, '0');
  }
}

// Export singleton instance
export const SatspathService = new SatspathServiceClass();

// Also export the class for testing
export { SatspathServiceClass };

// Type exports for consumers
export type {
  WasmPaymentMethod,
  WasmPaymentProfile,
  WasmSignedPaymentProfile,
  WasmQuoteResponse,
  WasmQuoteRecipient,
  WasmInvite,
  WasmFeeEstimate,
  WasmRouteRequest,
  WasmRouteQuote,
  WasmIdentity,
  SatspathConfig
};