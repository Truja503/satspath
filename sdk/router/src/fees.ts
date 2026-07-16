/**
 * Fee estimation — fetches from mempool.space
 * Mirrors Rust satspath-router/src/fees.rs
 */

import { FeeEstimate } from "./types";

const MEMPOOL_API = "https://mempool.space/api/v1/fees/recommended";

export async function fetchFeeEstimate(): Promise<FeeEstimate> {
  const response = await fetch(MEMPOOL_API);
  if (!response.ok) {
    throw new Error(`Failed to fetch fees: ${response.status}`);
  }
  const data = await response.json();
  return {
    fastest_fee: data.fastestFee,
    half_hour_fee: data.halfHourFee,
    hour_fee: data.hourFee,
    economy_fee: data.economyFee,
    minimum_fee: data.minimumFee,
  };
}

/** Fallback fees for offline/testing */
export const FALLBACK_FEES: FeeEstimate = {
  fastest_fee: 5,
  half_hour_fee: 4,
  hour_fee: 3,
  economy_fee: 2,
  minimum_fee: 1,
};