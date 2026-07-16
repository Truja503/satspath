/**
 * Core TypeScript types for SatsPath — mirrors Rust satspath-core exactly
 * These must match the Rust serde JSON output byte-for-byte
 */

export type BitcoinNetwork = "mainnet" | "testnet" | "regtest";

/** Payment method — tagged union matching Rust PaymentMethod enum */
export type TypedPaymentMethod =
  | OnchainMethod
  | LightningMethod
  | ArkMethod;

export interface OnchainMethod {
  type: "Onchain";
  label: string;
  network: BitcoinNetwork;
  address?: string;
  silent_payment_pubkey?: string;
  pubkey_hint?: string;
  descriptor_hint?: string;
  address_list: string[];
}

export interface LightningMethod {
  type: "Lightning";
  label: string;
  lightning_address?: string;
  lnurl?: string;
  bolt12?: string;
  receiver_pubkey?: string;
}

export interface ArkMethod {
  type: "Ark";
  label: string;
  server: string;
  pubkey: string;
  vtxo_pointer?: string;
  opaque_uri?: string;
  proof?: ArkOwnershipProof;
  expires_at?: number;
}

export interface ArkOwnershipProof {
  type: string;
  data: Record<string, unknown>;
}

/** Payment profile — matches Rust PaymentProfile */
export interface PaymentProfile {
  alias: string;
  identity_pubkey: string;
  methods: TypedPaymentMethod[];
  updated_at: number;
  expires_at?: number;
  sequence?: number;
  preferences: string[];
  nonce?: string;
  rotation?: KeyRotation;
  method_verifications: MethodVerification[];
}

/** Signed payment profile — matches Rust SignedPaymentProfile */
export interface SignedPaymentProfile {
  profile: PaymentProfile;
  signature: string; // hex-encoded Schnorr signature (64 bytes)
}

/** Key rotation — matches Rust KeyRotation */
export interface KeyRotation {
  new_identity_pubkey: string;
  rotation_time: number;
  previous_signature: string;
}

/** Method ownership verification — matches Rust MethodVerification */
export interface MethodVerification {
  method_descriptor: string;
  proof_type: string;
  proof_data: string;
  verified_at: number;
}

/** Resolver interface — mirrors Rust ProfileResolver trait */
export interface ProfileResolver {
  resolve_alias(alias: string): Promise<SignedPaymentProfile>;
}

/** Errors */
export class SatsPathError extends Error {
  constructor(message: string, public code: string) {
    super(message);
    this.name = "SatsPathError";
  }
}

export class AliasNotFoundError extends SatsPathError {
  constructor(alias: string) {
    super(`Alias not found: ${alias}`, "ALIAS_NOT_FOUND");
    this.name = "AliasNotFoundError";
  }
}

export class InvalidSignatureError extends SatsPathError {
  constructor(message: string) {
    super(message, "INVALID_SIGNATURE");
    this.name = "InvalidSignatureError";
  }
}

export class ProfileExpiredError extends SatsPathError {
  constructor() {
    super("Profile expired", "PROFILE_EXPIRED");
    this.name = "ProfileExpiredError";
  }
}

/** Invite for unregistered users — matches Rust Invite */
export interface Invite {
  alias_hash: string;
  amount_sats: number;
  created_at: number;
  expires_at: number;
  claim_url: string;
  warning: string;
  sender_signature?: string;
  sender_pubkey?: string;
}

/** Invite record for local storage — matches Rust InviteRecord */
export interface InviteRecord {
  invite_id: string;
  identifier_hash: string;
  display_hint: string;
  amount_sats: number;
  memo?: string;
  sender_fingerprint: string;
  status: "waiting_for_claim" | "email_sent" | "claimed_with_public_profile" | "expired" | "cancelled";
  created_at: number;
  expires_at: number;
}

/** Privacy helpers — matches Rust privacy module */
export function identifierHash(alias: string): string {
  // SHA-256 of lowercase alias
  // In browser, use crypto.subtle.digest
  // This is a placeholder - real impl uses @noble/hashes
  let hash = 0;
  const str = alias.toLowerCase().trim();
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash) + str.charCodeAt(i);
    hash |= 0;
  }
  return Array.from(new Uint8Array(32), (_, i) => (hash >> (i % 8)) & 0xff)
    .map(b => b.toString(16).padStart(2, "0"))
    .join("");
}

export function maskIdentifier(alias: string): string {
  const [local, domain] = alias.split("@");
  if (!domain) return "***";
  const masked = local.length > 2 
    ? local.slice(0, 1) + "*".repeat(local.length - 2) + local.slice(-1)
    : "***";
  return `${masked}@${domain}`;
}

/** Canonical JSON — placeholder for canonical-json serialization */
export function canonicalJsonStringify(obj: unknown): string {
  // In production, use canonical-json package
  return JSON.stringify(obj);
}