// SatsPath TypeScript types for Arkade Wallet
// These mirror the Rust types from satspath-core

export type BitcoinNetwork = 'mainnet' | 'testnet' | 'regtest';

export interface PaymentMethod {
  type: 'onchain' | 'lightning' | 'ark';
  label: string;
}

export interface OnchainMethod extends PaymentMethod {
  type: 'onchain';
  network: BitcoinNetwork;
  address?: string;
  silent_payment_pubkey?: string;
  pubkey_hint?: string;
  descriptor_hint?: string;
  address_list?: string[];
}

export interface LightningMethod extends PaymentMethod {
  type: 'lightning';
  lightning_address?: string;
  lnurl?: string;
  bolt12?: string;
  receiver_pubkey?: string;
}

export interface ArkMethod extends PaymentMethod {
  type: 'ark';
  server: string;
  pubkey: string;
  vtxo_pointer?: string;
  opaque_uri?: string;
  proof?: ArkOwnershipProof;
  expires_at?: number;
}

export type TypedPaymentMethod = OnchainMethod | LightningMethod | ArkMethod;

export interface ArkOwnershipProof {
  proof_type: string;
  data: Record<string, any>;
}

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

export interface SignedPaymentProfile {
  profile: PaymentProfile;
  signature: string;
}

export interface KeyRotation {
  new_identity_pubkey: string;
  rotation_time: number;
  previous_signature: string;
}

export interface MethodVerification {
  method_descriptor: string;
  proof_type: string;
  proof_data: string;
  verified_at: number;
}

export interface PaymentRequest {
  version: number;
  alias: string;
  amount_sats?: number;
  memo?: string;
  expires_at?: number;
  profile_hint?: string;
}

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

export interface InviteRecord {
  invite_id: string;
  identifier_hash: string;
  display_hint: string;
  amount_sats: number;
  memo?: string;
  sender_fingerprint: string;
  status: 'waiting_for_claim' | 'email_sent' | 'claimed_with_public_profile' | 'expired' | 'cancelled';
  created_at: number;
  expires_at: number;
}

export type InviteStatus = 
  | 'waiting_for_claim'
  | 'email_sent'
  | 'claimed_with_public_profile'
  | 'expired'
  | 'cancelled';

export interface ProfileResolver {
  resolve_alias(alias: string): Promise<SignedPaymentProfile>;
}

export interface ChainResolver extends ProfileResolver {
  push(resolver: ProfileResolver): ChainResolver;
}

export interface AliasNotFoundError extends Error {
  alias: string;
}

export class SatsPathError extends Error {
  constructor(message: string, public code: string) {
    super(message);
    this.name = 'SatsPathError';
  }
}

export class InvalidSignatureError extends SatsPathError {
  constructor(message: string) {
    super(message, 'INVALID_SIGNATURE');
    this.name = 'InvalidSignatureError';
  }
}

export class ProfileExpiredError extends SatsPathError {
  constructor(message: string) {
    super(message, 'PROFILE_EXPIRED');
    this.name = 'ProfileExpiredError';
  }
}

// ===== Quote/Router Types =====

export interface FeeEstimate {
  fastest_fee: number;
  half_hour_fee: number;
  hour_fee: number;
  economy_fee: number;
  minimum_fee: number;
}

export interface RouteRequest {
  alias: string;
  amount_sats: number;
  signed_profile: SignedPaymentProfile;
  urgency: 'low' | 'normal' | 'high';
  max_fee_sats?: number;
  max_fee_percent?: number;
}

export interface FeeRateSnapshot {
  fastest_sat_vb: number;
  half_hour_sat_vb: number;
  hour_sat_vb: number;
}

export interface RouteQuote {
  selected_method: TypedPaymentMethod;
  reason: string;
  estimated_fee_sats: number;
  estimated_confirmation: string;
  fee_snapshot?: FeeRateSnapshot;
  swap_directive: SwapDirective;
  execution: ExecutionMode;
  wallet_hint: string;
}

export interface SwapDirective {
  type: 'lightning_payment' | 'submarine_swap' | 'reverse_swap' | 'chain_swap' | 'ark_transfer' | 'arkade_manual';
  target_ln_address?: string;
  target_invoice?: string;
  target_address?: string;
  silent_payment_pubkey?: string;
  server?: string;
  pubkey?: string;
}

export type ExecutionMode = 
  | { type: 'preview' }
  | { type: 'mainnet_preview' }
  | { type: 'testnet_experimental' }
  | { type: 'manual_wallet' };

export interface QuoteRecipient {
  alias: string;
  verified: boolean;
  profile_signature_verified: boolean;
  identifier_verified: boolean;
  identifier_verification: string;
  fingerprint?: string;
}

export type QuoteResponse =
  | { status: 'ok'; recipient: QuoteRecipient; selected_method: TypedPaymentMethod; fee_sats: number; eta: string; reason: string; qr: string; execution: ExecutionMode; wallet_hint: string }
  | { status: 'not_registered'; invite: Invite }
  | { status: 'no_route'; reason: string }
  | { status: 'invalid_signature'; recipient: QuoteRecipient };

export type QuoteStatus = 'ok' | 'not_registered' | 'no_route' | 'invalid_signature';

export const LIGHTNING_THRESHOLD_SATS = 100_000;
export const MAX_ONCHAIN_FEE_SAT_VB = 10;
export const ONCHAIN_FEE_BUFFER = 1.10;