/**
 * Core Router — selects the best payment rail
 * Mirrors Rust satspath-router/src/router.rs select_route exactly
 */

import type { 
  TypedPaymentMethod, 
  OnchainMethod, 
  LightningMethod, 
  ArkMethod,
  RouteRequest, 
  RouteQuote, 
  FeeEstimate,
  ExecutionMode,
  SwapDirective
} from "@satspath/resolvers";

import { fetchFeeEstimate, FALLBACK_FEES } from "./fees";
import { 
  isLightningAvailable, 
  estimateLightningFee, 
  isOnchainAvailable, 
  getFirstOnchain, 
  estimateOnchainFee,
  isArkAvailable,
  getFirstArk,
  planArkRoute,
  type SenderCapabilities,
  type ArkRoutePlan
} from "./rails";

const LIGHTNING_THRESHOLD_SATS = 100_000;
const ONCHAIN_FEE_THRESHOLD_SAT_VB = 10;

/** Urgency levels match Rust */
export type PaymentUrgency = "low" | "normal" | "high";

export function selectFeeRate(urgency: PaymentUrgency, fees: FeeEstimate): number {
  switch (urgency) {
    case "high": return fees.fastest_fee;
    case "normal": return fees.half_hour_fee;
    case "low": return fees.hour_fee;
  }
}

export function expectedConfirmation(urgency: PaymentUrgency): string {
  switch (urgency) {
    case "high": return "~10 min";
    case "normal": return "~30 min";
    case "low": return "~60 min";
  }
}

/** Main router function — exact port of Rust select_route_with_fees */
export function selectRoute(req: RouteRequest, fees: FeeEstimate): RouteQuote {
  const { signed_profile, amount_sats, urgency } = req;
  const methods = signed_profile.profile.methods;

  // 1. LIGHTNING — evaluated FIRST, independent of fees
  if (amount_sats < LIGHTNING_THRESHOLD_SATS) {
    for (const method of methods) {
      if (method.type === "Lightning" && isLightningAvailable(method, amount_sats)) {
        const lnMethod = method as LightningMethod;
        return {
          selected_method: lnMethod,
          estimated_fee_sats: estimateLightningFee(amount_sats),
          estimated_confirmation: "instant",
          reason: `Amount (${amount_sats} sats) is below ${LIGHTNING_THRESHOLD_SATS} sats threshold and Lightning is available.`,
          fee_snapshot: undefined,
          swap_directive: {
            type: "LightningPayment",
            target_ln_address: lnMethod.lightning_address
          },
          execution: { type: "ManualWallet" },
          wallet_hint: "Use any Lightning wallet (LDK, Breez, Phoenix, etc.) to pay the invoice."
        };
      }
    }
  }

  // 2. ON-CHAIN — check fee rate
  const selectedFeeRateRaw = selectFeeRate(urgency, fees);
  const selectedFeeRate = Math.ceil(selectedFeeRateRaw * 1.1); // 10% buffer
  const expectedConf = expectedConfirmation(urgency);

  if (isOnchainAvailable(methods) && selectedFeeRate <= ONCHAIN_FEE_THRESHOLD_SAT_VB) {
    const onchainMethod = getFirstOnchain(methods)!;
    return {
      selected_method: onchainMethod,
      estimated_fee_sats: estimateOnchainFee(amount_sats, selectedFeeRate),
      estimated_confirmation: expectedConf,
      reason: `Fee (${selectedFeeRate} sat/vB) is cheap. Confirmation expected in ${expectedConf}.`,
      fee_snapshot: fees,
      swap_directive: {
        type: "ChainSwap",
        target_address: onchainMethod.address,
        silent_payment_pubkey: onchainMethod.silent_payment_pubkey
      },
      execution: { type: "ManualWallet" },
      wallet_hint: "Use any BIP-21 compatible wallet (BlueWallet, Sparrow, Electrum, etc.)"
    };
  }

  // 3. ARK — fallback when on-chain fees are high
  if (isArkAvailable(methods)) {
    const arkMethod = getFirstArk(methods)!;
    const reason = isOnchainAvailable(methods)
      ? `On-chain fee (${selectedFeeRate} sat/vB) exceeds ${ONCHAIN_FEE_THRESHOLD_SAT_VB} sat/vB. Falling back to Ark.`
      : "No Lightning (amount above threshold) or on-chain method. Using Ark.";

    const isArkade = !!arkMethod.opaque_uri;
    
    return {
      selected_method: arkMethod,
      estimated_fee_sats: 1, // Ark server absorbs fees
      estimated_confirmation: "near-instant via Ark round",
      reason,
      fee_snapshot: fees,
      swap_directive: isArkade 
        ? { type: "ArkadeManual" }
        : { type: "ArkTransfer", server: arkMethod.server, pubkey: arkMethod.pubkey },
      execution: isArkade ? { type: "ManualWallet" } : { type: "TestnetExperimental" },
      wallet_hint: isArkade 
        ? "Open Arkade.money wallet to complete the VTXO transfer."
        : "Ark VTXO transfer (testnet preview)."
    };
  }

  // 4. LAST RESORT: on-chain even with high fees
  if (isOnchainAvailable(methods)) {
    const onchainMethod = getFirstOnchain(methods)!;
    return {
      selected_method: onchainMethod,
      estimated_fee_sats: estimateOnchainFee(amount_sats, selectedFeeRate),
      estimated_confirmation: expectedConf,
      reason: `⚠ On-chain fee rate is high (${selectedFeeRate} sat/vB) but no alternative rail is available. Consider waiting for lower fees.`,
      fee_snapshot: fees,
      swap_directive: {
        type: "ChainSwap",
        target_address: onchainMethod.address,
        silent_payment_pubkey: onchainMethod.silent_payment_pubkey
      },
      execution: { type: "ManualWallet" },
      wallet_hint: "Use any BIP-21 compatible wallet."
    };
  }

  // 5. NO ROUTE
  throw new Error(
    `No usable rail for ${amount_sats} sats to ${req.alias}. ` +
    `Lightning: ${amount_sats >= LIGHTNING_THRESHOLD_SATS ? "amount above threshold" : "no LN method"}. ` +
    `On-chain: fee ${selectedFeeRate} sat/vB > ${ONCHAIN_FEE_THRESHOLD_SAT_VB} sat/vB. ` +
    `Ark: ${isArkAvailable(methods) ? "available" : "no method configured"}.`
  );
}

/** Async wrapper that fetches live fees */
export async function selectRouteLive(req: RouteRequest): Promise<RouteQuote> {
  let fees: FeeEstimate;
  try {
    fees = await fetchFeeEstimate();
  } catch {
    fees = FALLBACK_FEES;
  }
  return selectRoute(req, fees);
}