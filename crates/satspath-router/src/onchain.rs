use satspath_core::PaymentMethod;

use crate::fees::FeeEstimate;

/// Estimate on-chain fee in sats for a standard P2WPKH transaction (~141 vBytes).
pub fn estimate_onchain_fee_sats(fee_rate_sat_vb: u64) -> u64 {
    141 * fee_rate_sat_vb
}

/// Check whether an on-chain address is available in the method list.
pub fn is_onchain_available(methods: &[PaymentMethod]) -> bool {
    methods
        .iter()
        .any(|m| matches!(m, PaymentMethod::Onchain { .. }))
}

/// Find the first available on-chain method.
pub fn first_onchain_method(methods: &[PaymentMethod]) -> Option<&PaymentMethod> {
    methods
        .iter()
        .find(|m| matches!(m, PaymentMethod::Onchain { .. }))
}

/// On-chain is viable only if the next-block fee (fastestFee) is cheap.
///
/// We use `fastestFee` — not hourFee — because we only route on-chain
/// when confirmation can happen in <10 minutes (next block).
/// If even the next-block rate is expensive, we fall back to Ark.
///
/// Threshold: 20 sat/vB — reasonable for "cheap next-block" on mainnet.
pub fn is_onchain_fee_acceptable(estimate: &FeeEstimate) -> bool {
    estimate.fastest_fee <= 20
}
