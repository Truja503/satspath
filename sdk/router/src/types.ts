/**
 * SatsPath Router Types — mirrors Rust satspath-router exactly
 * Keep in sync with crates/satspath-router/src/
 */

import type { TypedPaymentMethod } from "@satspath/resolvers";

// ===== Fee Estimates =====

export interface FeeEstimate {
  fastest_fee: number;      // sat/vB - next block
  half_hour_fee: number;    // sat/vB - ~30 min
  hour_fee: number;         // sat/vB - ~1 hour
  economy_fee: number;      // sat/vB - ~24 hours
  minimum_fee: number;      // sat/vB - absolute minimum
}

// ===== Routing Request =====

export interface RouteRequest {
  alias: string;
  amount_sats: number;
  signed_profile: SignedPaymentProfile;
  urgency: "low" | "normal" | "high";
  max_fee_sats?: number;
  max_fee_percent?: number;
}

// Need to import from resolvers
export type SignedPaymentProfile = import("@satspath/resolvers").SignedPaymentProfile;

// ===== Route Quote (what the router returns) =====

export interface RouteQuote {
  selected_method: TypedPaymentMethod;
  estimated_fee_sats: number;
  estimated_confirmation: string;
  reason: string;
  fee_snapshot?: FeeRateSnapshot;
  swap_directive: SwapDirective;
  execution: ExecutionMode;
  wallet_hint: string;
}

export interface FeeRateSnapshot {
  fastest_sat_vb: number;
  half_hour_sat_vb: number;
  hour_sat_vb: number;
}

// ===== Swap Directives (for experimental swap engine) =====

export type SwapDirective =
  | { type: "LightningPayment"; target_ln_address?: string }
  | { type: "SubmarineSwap"; target_invoice?: string }
  | { type: "ReverseSwap"; target_address?: string; silent_payment_pubkey?: string }
  | { type: "ChainSwap"; target_address?: string; silent_payment_pubkey?: string }
  | { type: "ArkTransfer"; server: string; pubkey: string }
  | { type: "ArkadeManual" };

// ===== Execution Modes =====

export type ExecutionMode =
  | { type: "Preview" }
  | { type: "MainnetPreview" }
  | { type: "TestnetExperimental" }
  | { type: "ManualWallet" };

// ===== Quote Response (public UX contract) =====

export interface QuoteRecipient {
  alias: string;
  verified: boolean;
  profile_signature_verified: boolean;
  identifier_verified: boolean;
  identifier_verification: string;
  fingerprint?: string;
}

export type QuoteResponse =
  | { status: "ok"; recipient: QuoteRecipient; selected_method: TypedPaymentMethod; fee_sats: number; eta: string; reason: string; qr: string; execution: ExecutionMode; wallet_hint: string }
  | { status: "not_registered"; invite: Invite }
  | { status: "no_route"; reason: string }
  | { status: "invalid_signature"; recipient: QuoteRecipient };

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

// ===== Ark Route Planning =====

export interface ArkRoutePlan {
  kind: "ArkToArk" | "ArkToLightning" | "LightningToArk" | "ArkToOnchain" | "OnchainToArk";
  requires_swap: boolean;
  requires_boltz: boolean;
  requires_ark_bridge: boolean;
  testnet_only: boolean;
  reason: string;
}

export interface SenderCapabilities {
  ark_server?: string;
  has_lightning: boolean;
  has_onchain: boolean;
}

// ===== Constants =====

export const LIGHTNING_THRESHOLD_SATS = 100_000;
export const MAX_ONCHAIN_FEE_SAT_VB = 10;
export const ONCHAIN_FEE_BUFFER = 1.10; // 10% buffer on fee estimate