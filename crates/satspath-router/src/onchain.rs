use satspath_core::PaymentMethod;

use crate::fees::FeeEstimate;

/// Estimate on-chain fee in sats for a standard P2WPKH transaction (~141 vBytes).
pub fn estimate_onchain_fee_sats(fee_rate_sat_vb: u64) -> u64 {
    141 * fee_rate_sat_vb
}

/// Check whether an on-chain address is available in the method list.
pub fn is_onchain_available(methods: &[PaymentMethod]) -> bool {
    methods.iter().any(|m| matches!(m, PaymentMethod::Onchain { .. }))
}

/// Find the first available on-chain method.
pub fn first_onchain_method(methods: &[PaymentMethod]) -> Option<&PaymentMethod> {
    methods
        .iter()
        .find(|m| matches!(m, PaymentMethod::Onchain { .. }))
}

/// Decide whether on-chain is viable given current fee environment.
///
/// Uses `hourFee` as the reference rate. Threshold is 10 sat/vB.
pub fn is_onchain_fee_acceptable(estimate: &FeeEstimate) -> bool {
    estimate.hour_fee <= 10
}
