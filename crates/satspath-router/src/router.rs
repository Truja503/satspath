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

/// The selected payment rail and associated metadata.
#[derive(Debug, Clone)]
pub struct RouteQuote {
    pub selected_method: PaymentMethod,
    pub reason: String,
    pub estimated_fee_sats: Option<u64>,
    pub estimated_confirmation: Option<String>,
    /// Live fee snapshot used for the routing decision (for display purposes).
    pub fee_snapshot: Option<FeeRateSnapshot>,
}

/// A snapshot of the mempool fee rates at routing time.
#[derive(Debug, Clone)]
pub struct FeeRateSnapshot {
    pub fastest_sat_vb: u64,
    pub half_hour_sat_vb: u64,
    pub hour_sat_vb: u64,
}

/// Priority:
///   1. Lightning  — if amount < 100 000 sats and Lightning method exists
///   2. On-chain   — if fastestFee ≤ 20 sat/vB (next block, ~10 min) and on-chain address exists
///   3. Ark        — if an Ark method exists
///   4. Error
pub async fn select_route(req: &RouteRequest) -> satspath_core::Result<RouteQuote> {
    let methods = &req.signed_profile.profile.methods;

    // 1. Lightning
    if req.amount_sats < LIGHTNING_THRESHOLD_SATS {
        if let Some(ln) = methods.iter().find(|m| is_lightning_available(m)) {
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
            });
        }
    }

    // 2. On-chain — check next-block fee for <10 min confirmation
    let fee_est = fetch_fee_estimate().await;
    let snapshot = FeeRateSnapshot {
        fastest_sat_vb: fee_est.fastest_fee,
        half_hour_sat_vb: fee_est.half_hour_fee,
        hour_sat_vb: fee_est.hour_fee,
    };

    if is_onchain_available(methods) && is_onchain_fee_acceptable(&fee_est) {
        let method = first_onchain_method(methods).unwrap().clone();
        let fee = estimate_onchain_fee_sats(fee_est.fastest_fee);
        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "Next-block fee ({} sat/vB) is cheap. Expected confirmation <10 min.",
                fee_est.fastest_fee
            ),
            estimated_fee_sats: Some(fee),
            estimated_confirmation: Some("~10 minutes (next block)".into()),
            fee_snapshot: Some(snapshot),
        });
    }

    // 3. Ark
    if is_ark_available(methods) {
        let method = first_ark_method(methods).unwrap().clone();
        let reason = if is_onchain_available(methods) {
            format!(
                "On-chain next-block fee ({} sat/vB) exceeds 20 sat/vB threshold. Falling back to Ark.",
                fee_est.fastest_fee
            )
        } else {
            "No Lightning or on-chain method available. Using Ark.".into()
        };
        return Ok(RouteQuote {
            selected_method: method,
            reason,
            estimated_fee_sats: Some(1),
            estimated_confirmation: Some("near-instant via Ark round".into()),
            fee_snapshot: Some(snapshot),
        });
    }

    Err(SatsPathError::NoRouteFound(format!(
        "No usable rail for {} sats to {}. \
         Lightning: below threshold or unavailable. \
         On-chain: next-block fee {} sat/vB > 20 sat/vB. \
         Ark: no method configured.",
        req.amount_sats, req.alias, fee_est.fastest_fee,
    )))
}

/// Synchronous route selection using a pre-fetched fee estimate (used in unit tests).
pub fn select_route_with_fees(
    req: &RouteRequest,
    fee_est: &FeeEstimate,
) -> satspath_core::Result<RouteQuote> {
    let methods = &req.signed_profile.profile.methods;

    if req.amount_sats < LIGHTNING_THRESHOLD_SATS {
        if let Some(ln) = methods.iter().find(|m| is_lightning_available(m)) {
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
        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "Next-block fee ({} sat/vB) is cheap. Expected confirmation <10 min.",
                fee_est.fastest_fee
            ),
            estimated_fee_sats: Some(fee),
            estimated_confirmation: Some("~10 minutes (next block)".into()),
            fee_snapshot: Some(snapshot),
        });
    }

    if is_ark_available(methods) {
        let method = first_ark_method(methods).unwrap().clone();
        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "On-chain next-block fee ({} sat/vB) exceeds 20 sat/vB. Falling back to Ark.",
                fee_est.fastest_fee
            ),
            estimated_fee_sats: Some(1),
            estimated_confirmation: Some("near-instant via Ark round".into()),
            fee_snapshot: Some(snapshot),
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
        FeeEstimate {
            fastest_fee: 5,
            half_hour_fee: 4,
            hour_fee: 3,
            economy_fee: 2,
            minimum_fee: 1,
        }
    }

    fn high_fees() -> FeeEstimate {
        FeeEstimate {
            fastest_fee: 50,
            half_hour_fee: 30,
            hour_fee: 20,
            economy_fee: 15,
            minimum_fee: 10,
        }
    }

    fn make_profile(methods: Vec<PaymentMethod>) -> SignedPaymentProfile {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let profile = PaymentProfile {
            alias: "test@example.com".into(),
            identity_pubkey: pubkey_hex,
            methods,
            updated_at: 1_700_000_000,
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
            PaymentMethod::Onchain {
                label: "BTC".into(),
                address: "bc1q...".into(),
                pubkey_hint: None,
            },
        ]);
        let req = RouteRequest {
            alias: "test@example.com".into(),
            amount_sats: 21_000,
            signed_profile: signed,
        };
        let quote = select_route_with_fees(&req, &low_fees()).unwrap();
        assert!(matches!(quote.selected_method, PaymentMethod::Lightning { .. }));
    }

    #[test]
    fn chooses_onchain_for_large_amount_low_fees() {
        let signed = make_profile(vec![PaymentMethod::Onchain {
            label: "BTC".into(),
            address: "bc1q...".into(),
            pubkey_hint: None,
        }]);
        let req = RouteRequest {
            alias: "test@example.com".into(),
            amount_sats: 500_000,
            signed_profile: signed,
        };
        let quote = select_route_with_fees(&req, &low_fees()).unwrap();
        assert!(matches!(quote.selected_method, PaymentMethod::Onchain { .. }));
        // Should reference fastest_fee
        assert!(quote.reason.contains("5 sat/vB"));
    }

    #[test]
    fn falls_back_to_ark_when_fees_high() {
        let signed = make_profile(vec![
            PaymentMethod::Onchain {
                label: "BTC".into(),
                address: "bc1q...".into(),
                pubkey_hint: None,
            },
            PaymentMethod::Ark {
                label: "Ark".into(),
                server: "ark.example.com".into(),
                pubkey: "aabbcc".into(),
            },
        ]);
        let req = RouteRequest {
            alias: "test@example.com".into(),
            amount_sats: 500_000,
            signed_profile: signed,
        };
        let quote = select_route_with_fees(&req, &high_fees()).unwrap();
        assert!(matches!(quote.selected_method, PaymentMethod::Ark { .. }));
        assert!(quote.reason.contains("50 sat/vB"));
    }

    #[test]
    fn no_route_when_no_methods() {
        let signed = make_profile(vec![]);
        let req = RouteRequest {
            alias: "test@example.com".into(),
            amount_sats: 500_000,
            signed_profile: signed,
        };
        let err = select_route_with_fees(&req, &high_fees()).unwrap_err();
        assert!(matches!(err, SatsPathError::NoRouteFound(_)));
    }

    #[test]
    fn onchain_boundary_at_20_sat_vb() {
        let signed = make_profile(vec![PaymentMethod::Onchain {
            label: "BTC".into(),
            address: "bc1q...".into(),
            pubkey_hint: None,
        }]);
        let req = RouteRequest {
            alias: "test@example.com".into(),
            amount_sats: 500_000,
            signed_profile: signed.clone(),
        };

        // Exactly at threshold — should choose on-chain
        let at_threshold = FeeEstimate {
            fastest_fee: 20,
            half_hour_fee: 15,
            hour_fee: 10,
            economy_fee: 5,
            minimum_fee: 1,
        };
        let q = select_route_with_fees(&req, &at_threshold).unwrap();
        assert!(matches!(q.selected_method, PaymentMethod::Onchain { .. }));

        // One above — no on-chain method and no Ark → error
        let above_threshold = FeeEstimate {
            fastest_fee: 21,
            half_hour_fee: 16,
            hour_fee: 11,
            economy_fee: 6,
            minimum_fee: 2,
        };
        let err = select_route_with_fees(&req, &above_threshold).unwrap_err();
        assert!(matches!(err, SatsPathError::NoRouteFound(_)));
    }
}
