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

/// The specific swap operation needed to fulfill this route.
#[derive(Debug, Clone)]
pub enum SwapDirective {
    /// Direct Lightning payment (no swap needed if sender has LN).
    LightningPayment { target_invoice: Option<String> },
    /// Submarine Swap: L1/Ark → Lightning.
    SubmarineSwap { target_invoice: Option<String> },
    /// Reverse Swap: Lightning → L1.
    ReverseSwap { target_address: String },
    /// Chain Swap: Ark/L1 → L1/Ark.
    ChainSwap { target_address: String },
    /// Direct Ark VTXO transfer (no swap needed if sender is on same Ark server).
    ArkTransfer { server: String, pubkey: String },
}

/// The selected payment rail and associated metadata.
#[derive(Debug, Clone)]
pub struct RouteQuote {
    pub selected_method: PaymentMethod,
    pub reason: String,
    pub estimated_fee_sats: Option<u64>,
    pub estimated_confirmation: Option<String>,
    pub swap_directive: SwapDirective,
    pub dust_threshold_sats: u64,
}

/// Select the best available payment rail for a given request.
///
/// Priority:
///   1. Lightning  — if amount < 100 000 sats and Lightning method exists
///   2. On-chain   — if hourFee ≤ 10 sat/vB and an on-chain address exists
///   3. Ark        — if an Ark method exists
///   4. Error      — no suitable rail
pub async fn select_route(req: &RouteRequest) -> satspath_core::Result<RouteQuote> {
    let methods = &req.signed_profile.profile.methods;
    let fee_est = fetch_fee_estimate().await;

    // Calculate dust threshold based on fastest fee
    // Typical P2TR input is ~57.5 vBytes, P2WPKH is ~68 vBytes.
    // We use 68 as a conservative estimate.
    let dust_threshold_sats = fee_est.fastest_fee * 68;

    if req.amount_sats < dust_threshold_sats {
        return Err(SatsPathError::NoRouteFound(format!(
            "Amount {} sats is below the economic dust threshold of {} sats (fee rate: {} sat/vB).",
            req.amount_sats, dust_threshold_sats, fee_est.fastest_fee
        )));
    }

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
                // We assume sender is on L1/Ark and needs to pay LN receiver -> SubmarineSwap
                // Or if sender is on LN, it's a Direct LightningPayment.
                // For satspath MVP, we assume sender originates from on-chain/Ark -> Submarine.
                swap_directive: SwapDirective::SubmarineSwap { target_invoice: None },
                dust_threshold_sats,
            });
        }
    }

    // 2. On-chain — check mempool fee
    if is_onchain_available(methods) && is_onchain_fee_acceptable(&fee_est) {
        let method = first_onchain_method(methods).unwrap().clone();
        let fee = estimate_onchain_fee_sats(fee_est.hour_fee);
        
        let target_address = match &method {
            PaymentMethod::Onchain { address, .. } => address.clone(),
            _ => unreachable!(),
        };

        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "On-chain fee is acceptable ({} sat/vB ≤ 10 sat/vB threshold).",
                fee_est.hour_fee
            ),
            estimated_fee_sats: Some(fee),
            estimated_confirmation: Some("~60 minutes".into()),
            swap_directive: SwapDirective::ChainSwap { target_address },
            dust_threshold_sats,
        });
    }

    // 3. Ark
    if is_ark_available(methods) {
        let method = first_ark_method(methods).unwrap().clone();
        
        let (server, pubkey) = match &method {
            PaymentMethod::Ark { server, pubkey, .. } => (server.clone(), pubkey.clone()),
            _ => unreachable!(),
        };

        return Ok(RouteQuote {
            selected_method: method,
            reason: "Ark selected as fallback (Lightning unavailable, on-chain fees too high)."
                .into(),
            estimated_fee_sats: Some(1),
            estimated_confirmation: Some("near-instant via Ark round".into()),
            swap_directive: SwapDirective::ArkTransfer { server, pubkey },
            dust_threshold_sats,
        });
    }

    Err(SatsPathError::NoRouteFound(format!(
        "No usable payment rail found for {} sats to {}. \
         Checked: Lightning (amount threshold), On-chain (fee: {} sat/vB), Ark.",
        req.amount_sats,
        req.alias,
        fee_est.hour_fee,
    )))
}

/// Synchronous route selection using a pre-fetched fee estimate (for tests).
pub fn select_route_with_fees(
    req: &RouteRequest,
    fee_est: &FeeEstimate,
) -> satspath_core::Result<RouteQuote> {
    let methods = &req.signed_profile.profile.methods;

    let dust_threshold_sats = fee_est.fastest_fee * 68;
    if req.amount_sats < dust_threshold_sats {
        return Err(SatsPathError::NoRouteFound(format!(
            "Amount {} sats is below the economic dust threshold of {} sats (fee rate: {} sat/vB).",
            req.amount_sats, dust_threshold_sats, fee_est.fastest_fee
        )));
    }

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
                swap_directive: SwapDirective::SubmarineSwap { target_invoice: None },
                dust_threshold_sats,
            });
        }
    }

    if is_onchain_available(methods) && is_onchain_fee_acceptable(fee_est) {
        let method = first_onchain_method(methods).unwrap().clone();
        let fee = estimate_onchain_fee_sats(fee_est.hour_fee);
        
        let target_address = match &method {
            PaymentMethod::Onchain { address, .. } => address.clone(),
            _ => unreachable!(),
        };

        return Ok(RouteQuote {
            selected_method: method,
            reason: format!(
                "On-chain fee is acceptable ({} sat/vB ≤ 10 sat/vB threshold).",
                fee_est.hour_fee
            ),
            estimated_fee_sats: Some(fee),
            estimated_confirmation: Some("~60 minutes".into()),
            swap_directive: SwapDirective::ChainSwap { target_address },
            dust_threshold_sats,
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
            reason: "Ark selected as fallback.".into(),
            estimated_fee_sats: Some(1),
            estimated_confirmation: Some("near-instant via Ark round".into()),
            swap_directive: SwapDirective::ArkTransfer { server, pubkey },
            dust_threshold_sats,
        });
    }

    Err(SatsPathError::NoRouteFound(format!(
        "No usable payment rail found for {} sats to {}.",
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
        let signed = make_profile(vec![
            PaymentMethod::Onchain {
                label: "BTC".into(),
                address: "bc1q...".into(),
                pubkey_hint: None,
            },
        ]);
        let req = RouteRequest {
            alias: "test@example.com".into(),
            amount_sats: 500_000,
            signed_profile: signed,
        };
        let quote = select_route_with_fees(&req, &low_fees()).unwrap();
        assert!(matches!(quote.selected_method, PaymentMethod::Onchain { .. }));
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
}
