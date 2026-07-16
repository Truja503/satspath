/**
 * Payment rail detection helpers
 * Mirrors Rust satspath-router/src/lightning.ts, onchain.ts, ark.ts
 */

import type { TypedPaymentMethod, LightningMethod, OnchainMethod, ArkMethod } from "@satspath/resolvers";

/** Check if a Lightning method is available for a given amount */
export function isLightningAvailable(method: TypedPaymentMethod, amountSats: number): boolean {
  if (method.type !== "Lightning") return false;
  
  // Check dust threshold if LNURL metadata available
  // For now, assume available if method exists
  return true;
}

/** Estimate Lightning fee (rough: 10 ppm + base) */
export function estimateLightningFee(amountSats: number): number {
  const ppmFee = Math.floor(amountSats * 10 / 1_000_000);
  return Math.max(1, ppmFee);
}

/** Check if on-chain method exists */
export function isOnchainAvailable(methods: TypedPaymentMethod[]): boolean {
  return methods.some(m => m.type === "Onchain");
}

/** Get first on-chain method */
export function getFirstOnchain(methods: TypedPaymentMethod[]): OnchainMethod | undefined {
  return methods.find(m => m.type === "Onchain") as OnchainMethod | undefined;
}

/** Estimate on-chain fee */
export function estimateOnchainFee(amountSats: number, feeRateSatVb: number): number {
  // ~140 vbytes for 1-input, 2-output taproot transaction
  const vsize = 140;
  return Math.ceil(vsize * feeRateSatVb);
}

/** Check if Ark method exists */
export function isArkAvailable(methods: TypedPaymentMethod[]): boolean {
  return methods.some(m => m.type === "Ark");
}

/** Get first Ark method */
export function getFirstArk(methods: TypedPaymentMethod[]): ArkMethod | undefined {
  return methods.find(m => m.type === "Ark") as ArkMethod | undefined;
}

/** Plan Ark route based on sender/receiver capabilities
 * Mirrors Rust satspath-router/src/ark_routes.rs plan_ark_route
 */
export interface SenderCapabilities {
  ark_server?: string;
  has_lightning: boolean;
  has_onchain: boolean;
}

export type ArkRouteKind = 
  | "ArkToArk" 
  | "ArkToLightning" 
  | "LightningToArk" 
  | "ArkToOnchain" 
  | "OnchainToArk";

export interface ArkRoutePlan {
  kind: ArkRouteKind;
  requires_swap: boolean;
  requires_boltz: boolean;
  requires_ark_bridge: boolean;
  testnet_only: boolean;
  reason: string;
}

export function planArkRoute(
  sender: SenderCapabilities,
  receiverProfile: { methods: TypedPaymentMethod[] }
): ArkRoutePlan | null {
  const receiverArk = getFirstArk(receiverProfile.methods);
  const receiverLightning = receiverProfile.methods.some(m => m.type === "Lightning");
  const receiverOnchain = receiverProfile.methods.some(m => m.type === "Onchain");

  // Same Ark server = direct VTXO transfer
  if (sender.ark_server && receiverArk && sender.ark_server === receiverArk.server) {
    return {
      kind: "ArkToArk",
      requires_swap: false,
      requires_boltz: false,
      requires_ark_bridge: true,
      testnet_only: true,
      reason: "Sender and receiver use the same Ark server; direct VTXO transfer intent."
    };
  }

  // Sender has Ark, receiver has Lightning = offboard via Boltz
  if (sender.ark_server && receiverLightning) {
    return {
      kind: "ArkToLightning",
      requires_swap: true,
      requires_boltz: true,
      requires_ark_bridge: true,
      testnet_only: true,
      reason: "Sender has Ark and receiver has Lightning; requires swap/offboard path."
    };
  }

  // Sender has Lightning, receiver has Ark = onboard via Boltz
  if (sender.has_lightning && receiverArk) {
    return {
      kind: "LightningToArk",
      requires_swap: true,
      requires_boltz: true,
      requires_ark_bridge: true,
      testnet_only: true,
      reason: "Sender has Lightning and receiver has Ark; requires onboard/reverse path."
    };
  }

  // Sender has Ark, receiver has on-chain = offboard
  if (sender.ark_server && receiverOnchain) {
    return {
      kind: "ArkToOnchain",
      requires_swap: true,
      requires_boltz: true,
      requires_ark_bridge: true,
      testnet_only: true,
      reason: "Sender has Ark and receiver has on-chain; requires offboard path."
    };
  }

  // Sender has on-chain, receiver has Ark = onboard
  if (sender.has_onchain && receiverArk) {
    return {
      kind: "OnchainToArk",
      requires_swap: true,
      requires_boltz: true,
      requires_ark_bridge: true,
      testnet_only: true,
      reason: "Sender has on-chain and receiver has Ark; requires onboard path."
    };
  }

  return null;
}