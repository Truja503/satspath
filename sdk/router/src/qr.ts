/**
 * QR Payload Builder — builds payment URIs for each rail
 * Mirrors Rust satspath-router/src/quote_response.rs build_qr_payload
 */

import type { TypedPaymentMethod } from "@satspath/resolvers";

export function buildQrPayload(method: TypedPaymentMethod, amountSats: number): string {
  switch (method.type) {
    case "Lightning": {
      // Priority: LNURL > Lightning Address > BOLT12
      return method.lnurl || method.lightning_address || method.bolt12 || "";
    }
    case "Onchain": {
      // BIP-21 URI
      const target = method.silent_payment_pubkey || method.address || "";
      const btc = (amountSats / 100_000_000).toFixed(8);
      return `bitcoin:${target}?amount=${btc}`;
    }
    case "Ark": {
      // Ark URI: ark:pubkey?server=...&amount=...
      return `ark:${encodeURIComponent(method.pubkey)}?server=${encodeURIComponent(method.server)}&amount=${amountSats}`;
    }
  }
}

/** Format satoshis as BTC with 8 decimals */
export function satsToBtc(amountSats: number): string {
  return (amountSats / 100_000_000).toFixed(8);
}

/** Estimate on-chain fee (rough: 140 vbytes for 1-in-2-out taproot) */
export function estimateOnchainFee(feeRateSatVb: number): number {
  const vsize = 140;
  return Math.ceil(vsize * feeRateSatVb);
}

/** Estimate Lightning fee (rough: 10ppm + base) */
export function estimateLightningFee(amountSats: number): number {
  return Math.max(1, Math.floor(amountSats * 0.0001)); // 10ppm
}