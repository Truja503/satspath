use satspath_core::{PaymentMethod, SatsPathError, SignedPaymentProfile};

use crate::ark::{first_ark_method, is_ark_available};
use crate::fees::{fetch_fee_estimate, FeeEstimate};
use crate::lightning::{estimate_lightning_fee_sats, is_lightning_available};
use crate::onchain::{
    estimate_onchain_fee_sats, first_onchain_method, is_onchain_available, is_onchain_fee_acceptable,
};

const LIGHTNING_THRESHOLD_SATS: u64 = 100_000;

/// A routing request: who to pay and how much.
#[derive(Debug, Clone)]
pub struct RouteRequest {
    pub alias: String,
    pub amount_sats: u64,
    pub signed_profile: SignedPaymentProfile,
}

/// Describes the specific execution path needed for the selected route.
/// Used by the experimental swap engine; safe path ignores this.
#[derive(Debug, Clone)]
pub enum SwapDirective {
    /// Direct Lightning payment via LNURL/Lightning Address.
    LightningPayment { target_ln_address: Option<String> },
    /// Submarine Swap: on-chain/Ark → Lightning (requires Boltz).
    SubmarineSwap { target_invoice: Option<String> },
    /// Reverse Swap: Lightning → on-chain (requires Boltz).
    ReverseSwap { target_address: String },
    /// Chain Swap: Ark/L1 → L1/Ark (requires Boltz).
    ChainSwap { target_address: String },
    /// Direct Ark VTXO transfer (same Ark server).
    ArkTransfer { server: String, pubkey: String },
}

/// Snapshot of live mempool fee rates used in the routing decision.
#[derive(Debug, Clone)]
pub struct FeeRateSnapshot {
    pub fastest_sat_vb: u64,
    pub half_hour_sat_vb: u64,
    pub hour_sat_vb: u64,
}

/// The selected payment rail and all information needed to execute it.
#[derive(Debug, Clone)]
pub struct RouteQuote {
    pub selected_method: PaymentMethod,
    pub reason: String,
    pub estimated_fee_sats: Option<u64>,
    pub estimated_confirmation: Option<String>,
    /// Live fee snapshot (present when mempool was queried).
    pub fee_snapshot: Option<FeeRateSnapshot>,
    /// Execution directive for the experimental swap engine.
    pub swap_directive: SwapDirective,
}

/// Select the best available payment rail.
///
/// Priority:
///   1. Lightning  — if amount < 100 000 sats and a Lightning method exists.
///                   NOTE: Lightning is checked BEFORE on-chain fees.
///                   The dust threshold must NOT block Lightning route selection.
///   2. On-chain   — if fastestFee ≤ 20 sat/vB (next block, <10 min).
///   3. Ark        — fallback when on-chain fees are too high.
///   4. Error      — no suitable rail found.
pub async fn select_route(req: &RouteRequest) -> satspath_core::Result<RouteQuote> {
    let methods = &req.signed_profile.profile.methods;

    // 1. Lightning — evaluated first, independent of fee environment.
    if req.amount_sats < LIGHTNING_THRESHOLD_SATS {
        if let Some(ln) = methods.iter().find(|m| is_lightning_available(m)) {
            let ln_address = match ln {
                PaymentMethod::Lightning { lightning_address, .. } => {
                    lightning_address.clone()
                }
                _ => None,
            };
            let fee = estimate_lightning_fee_sats(req.amount_sats);
            return Ok(RouteQuote {
                selected_method: ln.clone(),
                reason: format!(
                    "Amount ({} sats) is below {} sats threshold and Lightning is available.",
                    req.amount_sats, LIGHTNING_THRESHOLD_SATS
                ),
                estimated_fee_sats: Some(fee),
                estimated_confirmation: Some("instant".into()),
                fee_snapshot: None,
                swap_directive: SwapDirective::LightningPayment {
                    target_ln_address: ln_address,
                },
            });
        }
    }

    // Fetch live fees only when we need to evaluate on-chain or Ark.
    let fee_est = fetch_fee_estimate().await;
    let snapshot = FeeRateSnapshot {
        fastest_sat_vb: fee_est.fastest_fee,
        half_hour_sat_vb: fee_est.half_hour_fee,
        hour_sat_vb: fee_est.hour_fee,
    };

    // 2. On-chain — next-block fee must be ≤ 20 sat/vB (cheap AND fast, <10 min).
    if is_onchain_available(methods) && is_onchain_fee_acceptable(&fee_est) {
        let method = first_onchain_method(methods).unwrap().clone();
        let fee = estimate_onchain_fee_sats(fee_est.fastest_fee);
        let target_address = match &method {
            PaymentMethod::Onchain { address, .. } => address.clone(),
            _ => unreachable!(),
        };
        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "Next-block fee ({} sat/vB) is cheap. Confirmation expected in <10 min.",
                fee_est.fastest_fee
            ),
            estimated_fee_sats: Some(fee),
            estimated_confirmation: Some("~10 minutes (next block)".into()),
            fee_snapshot: Some(snapshot),
            swap_directive: SwapDirective::ChainSwap { target_address },
        });
    }

    // 3. Ark fallback.
    if is_ark_available(methods) {
        let method = first_ark_method(methods).unwrap().clone();
        let (server, pubkey) = match &method {
            PaymentMethod::Ark { server, pubkey, .. } => (server.clone(), pubkey.clone()),
            _ => unreachable!(),
        };
        let reason = if is_onchain_available(methods) {
            format!(
                "On-chain next-block fee ({} sat/vB) exceeds 20 sat/vB. Falling back to Ark.",
                fee_est.fastest_fee
            )
        } else {
            "No Lightning (amount above threshold) or on-chain method. Using Ark.".into()
        };
        return Ok(RouteQuote {
            selected_method: method,
            reason,
            estimated_fee_sats: Some(1),
            estimated_confirmation: Some("near-instant via Ark round".into()),
            fee_snapshot: Some(snapshot),
            swap_directive: SwapDirective::ArkTransfer { server, pubkey },
        });
    }

    Err(SatsPathError::NoRouteFound(format!(
        "No usable rail for {} sats to {}. \
         Lightning: {} sats threshold not met or no LN method. \
         On-chain: next-block fee {} sat/vB > 20 sat/vB. \
         Ark: no method configured.",
        req.amount_sats, req.alias, LIGHTNING_THRESHOLD_SATS, fee_est.fastest_fee,
    )))
}

/// Deterministic route selection for unit tests (pre-fetched fee estimate).
pub fn select_route_with_fees(
    req: &RouteRequest,
    fee_est: &FeeEstimate,
) -> satspath_core::Result<RouteQuote> {
    let methods = &req.signed_profile.profile.methods;

    // Lightning first — no fee check, no dust threshold.
    if req.amount_sats < LIGHTNING_THRESHOLD_SATS {
        if let Some(ln) = methods.iter().find(|m| is_lightning_available(m)) {
            let ln_address = match ln {
                PaymentMethod::Lightning { lightning_address, .. } => lightning_address.clone(),
                _ => None,
            };
            let fee = estimate_lightning_fee_sats(req.amount_sats);
            return Ok(RouteQuote {
                selected_method: ln.clone(),
                reason: format!(
                    "Amount ({} sats) is below {} sats threshold and Lightning is available.",
                    req.amount_sats, LIGHTNING_THRESHOLD_SATS
                ),
                estimated_fee_sats: Some(fee),
                estimated_confirmation: Some("instant".into()),
                fee_snapshot: None,
                swap_directive: SwapDirective::LightningPayment {
                    target_ln_address: ln_address,
                },
            });
        }
    }

    let snapshot = FeeRateSnapshot {
        fastest_sat_vb: fee_est.fastest_fee,
        half_hour_sat_vb: fee_est.half_hour_fee,
        hour_sat_vb: fee_est.hour_fee,
    };

    if is_onchain_available(methods) && is_onchain_fee_acceptable(fee_est) {
        let method = first_onchain_method(methods).unwrap().clone();
        let fee = estimate_onchain_fee_sats(fee_est.fastest_fee);
        let target_address = match &method {
            PaymentMethod::Onchain { address, .. } => address.clone(),
            _ => unreachable!(),
        };
        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "Next-block fee ({} sat/vB) is cheap. Confirmation expected in <10 min.",
                fee_est.fastest_fee
            ),
            estimated_fee_sats: Some(fee),
            estimated_confirmation: Some("~10 minutes (next block)".into()),
            fee_snapshot: Some(snapshot),
            swap_directive: SwapDirective::ChainSwap { target_address },
        });
    }

    if is_ark_available(methods) {
        let method = first_ark_method(methods).unwrap().clone();
        let (server, pubkey) = match &method {
            PaymentMethod::Ark { server, pubkey, .. } => (server.clone(), pubkey.clone()),
            _ => unreachable!(),
        };
        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "On-chain next-block fee ({} sat/vB) exceeds 20 sat/vB. Falling back to Ark.",
                fee_est.fastest_fee
            ),
            estimated_fee_sats: Some(1),
            estimated_confirmation: Some("near-instant via Ark round".into()),
            fee_snapshot: Some(snapshot),
            swap_directive: SwapDirective::ArkTransfer { server, pubkey },
        });
    }

    Err(SatsPathError::NoRouteFound(format!(
        "No usable rail for {} sats to {}.",
        req.amount_sats, req.alias,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use satspath_core::{
        crypto::{generate_identity_keypair, sign_profile},
        PaymentMethod, PaymentProfile,
    };

    fn low_fees() -> FeeEstimate {
        FeeEstimate { fastest_fee: 5, half_hour_fee: 4, hour_fee: 3, economy_fee: 2, minimum_fee: 1 }
    }

    fn high_fees() -> FeeEstimate {
        FeeEstimate { fastest_fee: 50, half_hour_fee: 30, hour_fee: 20, economy_fee: 15, minimum_fee: 10 }
    }

    fn make_profile(methods: Vec<PaymentMethod>) -> SignedPaymentProfile {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let profile = PaymentProfile {
            alias: "test@example.com".into(),
            identity_pubkey: pubkey_hex,
            methods,
            updated_at: 1_700_000_000,
            expires_at: None,
        };
        sign_profile(profile, &kp.secret_key).unwrap()
    }

    #[test]
    fn chooses_lightning_for_small_amount() {
        let signed = make_profile(vec![
            PaymentMethod::Lightning {
                label: "LN".into(),
                lnurl: None,
                lightning_address: Some("test@example.com".into()),
                bolt12: None,
            },
        ]);
        let req = RouteRequest { alias: "test@example.com".into(), amount_sats: 21_000, signed_profile: signed };
        let q = select_route_with_fees(&req, &low_fees()).unwrap();
        assert!(matches!(q.selected_method, PaymentMethod::Lightning { .. }));
    }

    #[test]
    fn lightning_not_blocked_by_fees() {
        // Even with extreme fees, Lightning for small amounts must still win.
        let signed = make_profile(vec![
            PaymentMethod::Lightning {
                label: "LN".into(),
                lnurl: None,
                lightning_address: Some("test@example.com".into()),
                bolt12: None,
            },
        ]);
        let extreme_fees = FeeEstimate { fastest_fee: 500, half_hour_fee: 400, hour_fee: 300, economy_fee: 200, minimum_fee: 100 };
        let req = RouteRequest { alias: "test@example.com".into(), amount_sats: 1_000, signed_profile: signed };
        let q = select_route_with_fees(&req, &extreme_fees).unwrap();
        assert!(matches!(q.selected_method, PaymentMethod::Lightning { .. }));
    }

    #[test]
    fn chooses_onchain_for_large_amount_low_fees() {
        let signed = make_profile(vec![PaymentMethod::Onchain {
            label: "BTC".into(),
            address: "bc1q...".into(),
            pubkey_hint: None,
        }]);
        let req = RouteRequest { alias: "test@example.com".into(), amount_sats: 500_000, signed_profile: signed };
        let q = select_route_with_fees(&req, &low_fees()).unwrap();
        assert!(matches!(q.selected_method, PaymentMethod::Onchain { .. }));
        assert!(q.reason.contains("5 sat/vB"));
    }

    #[test]
    fn falls_back_to_ark_when_fees_high() {
        let signed = make_profile(vec![
            PaymentMethod::Onchain { label: "BTC".into(), address: "bc1q...".into(), pubkey_hint: None },
            PaymentMethod::Ark { label: "Ark".into(), server: "ark.example.com".into(), pubkey: "aabbcc".into() },
        ]);
        let req = RouteRequest { alias: "test@example.com".into(), amount_sats: 500_000, signed_profile: signed };
        let q = select_route_with_fees(&req, &high_fees()).unwrap();
        assert!(matches!(q.selected_method, PaymentMethod::Ark { .. }));
        assert!(q.reason.contains("50 sat/vB"));
    }

    #[test]
    fn no_route_when_no_methods() {
        let signed = make_profile(vec![]);
        let req = RouteRequest { alias: "test@example.com".into(), amount_sats: 500_000, signed_profile: signed };
        assert!(matches!(
            select_route_with_fees(&req, &high_fees()).unwrap_err(),
            SatsPathError::NoRouteFound(_)
        ));
    }

    #[test]
    fn onchain_boundary_at_20_sat_vb() {
        let signed = make_profile(vec![PaymentMethod::Onchain {
            label: "BTC".into(),
            address: "bc1q...".into(),
            pubkey_hint: None,
        }]);
        let req = RouteRequest { alias: "test@example.com".into(), amount_sats: 500_000, signed_profile: signed };

        let at = FeeEstimate { fastest_fee: 20, half_hour_fee: 15, hour_fee: 10, economy_fee: 5, minimum_fee: 1 };
        assert!(matches!(select_route_with_fees(&req, &at).unwrap().selected_method, PaymentMethod::Onchain { .. }));

        let above = FeeEstimate { fastest_fee: 21, half_hour_fee: 16, hour_fee: 11, economy_fee: 6, minimum_fee: 2 };
        assert!(matches!(select_route_with_fees(&req, &above).unwrap_err(), SatsPathError::NoRouteFound(_)));
    }
}
