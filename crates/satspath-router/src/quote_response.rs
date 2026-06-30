//! The UX/backend **quote response contract**.
//!
//! One call — [`quote`] — resolves a recipient, verifies the signed profile,
//! checks expiry, selects a route, and builds a payment payload, returning a
//! single JSON-serializable [`QuoteResponse`] the frontend can render directly
//! by matching on `status`. The backend owns all protocol complexity.
//!
//! # Safety
//!
//! Responses carry **public data only**: payment profiles, public keys,
//! addresses, Lightning Addresses, LNURL/BOLT11 pointers, BIP-21 URIs, Ark
//! pointers, and invites. No funds are moved, nothing is signed or broadcast,
//! and no private key, seed, xprv, macaroon, cert, or secret is ever included —
//! [`build_qr_payload`] additionally screens payloads for private material.

use serde::{Deserialize, Serialize};

use satspath_core::{
    create_invite,
    crypto::{check_profile_expiry, fingerprint_pubkey, verify_signed_profile},
    profile::PaymentProfile,
    registry::Registry,
    resolver::{ChainResolver, ProfileResolver},
    resolvers::{bip353::Bip353Resolver, http::HttpResolver, nostr::NostrResolver},
    validation::assert_no_private_material,
    Invite, PaymentMethod, SatsPathError,
};

use crate::fees::FeeEstimate;
use crate::lightning::{fetch_invoice, fetch_lnurl_metadata};
use crate::router::{select_route, select_route_with_fees, RouteRequest};

/// Public, frontend-facing description of the recipient.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteRecipient {
    pub alias: String,
    pub verified: bool,
    pub fingerprint: Option<String>,
}

/// The single response the UX renders by matching on `status`.
///
/// Serializes with an internal `status` tag in `snake_case`
/// (`ok` / `not_registered` / `no_route` / `invalid_signature`).
///
/// The `Ok` variant is intentionally larger than the others: the response shape
/// is the public UX/JSON contract, which embeds `PaymentMethod` unchanged. We do
/// not box it (that would alter the Rust API for consumers); the wire shape is
/// identical either way.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QuoteResponse {
    Ok {
        recipient: QuoteRecipient,
        /// The selected payment method, embedded unchanged.
        selected_method: PaymentMethod,
        fee_sats: Option<u64>,
        eta: Option<String>,
        reason: String,
        qr: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        execution: Option<satspath_core::ExecutionMode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wallet_hint: Option<String>,
    },
    NotRegistered {
        invite: Invite,
    },
    NoRoute {
        reason: String,
    },
    InvalidSignature {
        recipient: QuoteRecipient,
    },
}

impl QuoteResponse {
    /// The `status` string this response serializes to (handy for callers/tests).
    pub fn status(&self) -> &'static str {
        match self {
            QuoteResponse::Ok { .. } => "ok",
            QuoteResponse::NotRegistered { .. } => "not_registered",
            QuoteResponse::NoRoute { .. } => "no_route",
            QuoteResponse::InvalidSignature { .. } => "invalid_signature",
        }
    }
}

// ─── Public entry points ──────────────────────────────────────────────────────

/// Resolve, verify, route, and build a payment payload for `recipient`.
///
/// Builds the default resolver chain (local registry → BIP-353 → HTTP → Nostr)
/// and, for a Lightning rail, best-effort fetches a real BOLT11 invoice so the
/// `qr` field is directly payable. Falls back to a safe Lightning pointer if the
/// fetch is unavailable.
///
/// Returns a [`QuoteResponse`] for every protocol outcome (found / not
/// registered / no route / invalid signature) — the frontend never sees a Rust
/// error type.
pub async fn quote(recipient: &str, amount_sats: u64) -> QuoteResponse {
    let resolver = default_resolver();
    quote_inner(&resolver, recipient, amount_sats, None, true).await
}

/// Like [`quote`], but against a caller-supplied resolver (e.g. an API server
/// that manages its own resolver chain). Uses live mempool fees and returns a
/// safe Lightning pointer rather than fetching an invoice in this layer.
pub async fn quote_with_resolver<R>(
    resolver: &R,
    recipient: &str,
    amount_sats: u64,
) -> QuoteResponse
where
    R: ProfileResolver + Sync + ?Sized,
{
    quote_inner(resolver, recipient, amount_sats, None, false).await
}

// ─── QR / payment payload ──────────────────────────────────────────────────────

/// Build a public payment payload for a method.
///
/// - **Lightning** — returns the best available pointer (LNURL, then Lightning
///   Address, then BOLT12). Fetching a concrete BOLT11 invoice happens in the
///   async [`quote`] flow (or the CLI/API layer), not here.
/// - **On-chain** — a BIP-21 URI: `bitcoin:<address>?amount=<btc>`.
/// - **Ark** — a SatsPath Ark pointer: `satspath:ark?server=…&pubkey=…&amount=…`
///   (preview only; execution is gated elsewhere behind explicit testnet flags).
pub fn build_qr_payload(method: &PaymentMethod, amount_sats: u64) -> satspath_core::Result<String> {
    let payload = match method {
        PaymentMethod::Lightning {
            lightning_address,
            lnurl,
            bolt12,
            ..
        } => lnurl
            .clone()
            .or_else(|| lightning_address.clone())
            .or_else(|| bolt12.clone())
            .ok_or_else(|| {
                SatsPathError::InvalidPaymentPointer(
                    "Lightning method has no address, LNURL, or BOLT12 pointer".into(),
                )
            })?,
        PaymentMethod::Onchain { address, .. } => {
            format!("bitcoin:{}?amount={}", address, sats_to_btc(amount_sats))
        }
        PaymentMethod::Ark { server, pubkey, .. } => format!(
            "satspath:ark?server={}&pubkey={}&amount={}",
            urlencoding::encode(server),
            urlencoding::encode(pubkey),
            amount_sats
        ),
    };

    // Defence in depth: a payment payload must never carry private material.
    assert_no_private_material(&payload)?;
    Ok(payload)
}

/// Format sats as a fixed 8-decimal BTC string, e.g. `21000 -> 0.00021000`.
fn sats_to_btc(amount_sats: u64) -> String {
    format!(
        "{}.{:08}",
        amount_sats / 100_000_000,
        amount_sats % 100_000_000
    )
}

// ─── Core orchestration ─────────────────────────────────────────────────────────

/// The shared quote pipeline.
///
/// `fees`: `Some` uses deterministic routing (no network — used by tests);
/// `None` fetches live mempool fees.
/// `fetch_ln_invoice`: when `true`, a Lightning rail is upgraded to a real
/// BOLT11 invoice (best effort).
async fn quote_inner<R>(
    resolver: &R,
    recipient: &str,
    amount_sats: u64,
    fees: Option<FeeEstimate>,
    fetch_ln_invoice: bool,
) -> QuoteResponse
where
    R: ProfileResolver + Sync + ?Sized,
{
    // 1. Resolve. Anything unresolvable becomes an invite — never a hard error.
    let signed = match resolver.resolve_alias(recipient).await {
        Ok(signed) => signed,
        Err(_) => {
            return QuoteResponse::NotRegistered {
                invite: create_invite(recipient, amount_sats),
            }
        }
    };

    // 2. Verify signature.
    let verified = verify_signed_profile(&signed).unwrap_or(false);
    let recipient_info = recipient_of(&signed.profile, verified);
    if !verified {
        return QuoteResponse::InvalidSignature {
            recipient: recipient_info,
        };
    }

    // 3. Expiry (mapped to no_route to keep the frontend contract to four states).
    if check_profile_expiry(&signed.profile).is_err() {
        return QuoteResponse::NoRoute {
            reason: "Profile expired.".into(),
        };
    }

    // 4. Route.
    let req = RouteRequest {
        alias: recipient.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let route = match fees {
        Some(fee_est) => select_route_with_fees(&req, &fee_est),
        None => select_route(&req).await,
    };
    let route = match route {
        Ok(route) => route,
        Err(SatsPathError::NoRouteFound(reason)) => return QuoteResponse::NoRoute { reason },
        Err(e) => {
            return QuoteResponse::NoRoute {
                reason: e.to_string(),
            }
        }
    };

    // 5. Build the payment payload.
    let mut qr = match build_qr_payload(&route.selected_method, amount_sats) {
        Ok(payload) => payload,
        Err(e) => {
            return QuoteResponse::NoRoute {
                reason: format!("could not build payment payload: {e}"),
            }
        }
    };

    // 6. Best-effort upgrade Lightning to a concrete BOLT11 invoice.
    if fetch_ln_invoice {
        if let PaymentMethod::Lightning {
            lightning_address: Some(addr),
            ..
        } = &route.selected_method
        {
            if let Ok(invoice) = fetch_real_invoice(addr, amount_sats).await {
                qr = invoice;
            }
        }
    }

    QuoteResponse::Ok {
        recipient: recipient_info,
        selected_method: route.selected_method,
        fee_sats: route.estimated_fee_sats,
        eta: route.estimated_confirmation,
        reason: route.reason,
        qr,
        execution: route.execution,
        wallet_hint: route.wallet_hint,
    }
}

fn recipient_of(profile: &PaymentProfile, verified: bool) -> QuoteRecipient {
    QuoteRecipient {
        alias: profile.alias.clone(),
        verified,
        fingerprint: fingerprint_pubkey(&profile.identity_pubkey).ok(),
    }
}

/// Two-step LNURL-pay: resolve metadata, then fetch a BOLT11 invoice.
async fn fetch_real_invoice(lightning_address: &str, amount_sats: u64) -> anyhow::Result<String> {
    let meta = fetch_lnurl_metadata(lightning_address).await?;
    fetch_invoice(&meta, amount_sats, None).await
}

/// The default resolver chain, mirroring the CLI: local registry, then public
/// resolvers. Registry lives in the always-gitignored `.satspath/`.
fn default_resolver() -> ChainResolver {
    let mut chain = ChainResolver::new();
    if let Ok(registry) = Registry::open(std::path::Path::new(".satspath")) {
        chain = chain.push(registry);
    }
    chain = chain.push(Bip353Resolver::new());
    chain = chain.push(HttpResolver::new());
    chain.push(NostrResolver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use satspath_core::crypto::{generate_identity_keypair, sign_profile};
    use satspath_core::{BitcoinNetwork, PaymentProfile, SignedPaymentProfile};

    // Deterministic fees: cheap next block (on-chain viable when a method exists).
    fn cheap_fees() -> FeeEstimate {
        FeeEstimate {
            fastest_fee: 5,
            half_hour_fee: 4,
            hour_fee: 3,
            economy_fee: 2,
            minimum_fee: 1,
        }
    }

    // Expensive next block (> 20 sat/vB): on-chain not acceptable.
    fn expensive_fees() -> FeeEstimate {
        FeeEstimate {
            fastest_fee: 35,
            half_hour_fee: 30,
            hour_fee: 25,
            economy_fee: 20,
            minimum_fee: 10,
        }
    }

    struct MockResolver {
        signed: Option<SignedPaymentProfile>,
    }

    #[async_trait]
    impl ProfileResolver for MockResolver {
        async fn resolve_alias(&self, alias: &str) -> satspath_core::Result<SignedPaymentProfile> {
            self.signed
                .clone()
                .ok_or_else(|| SatsPathError::AliasNotFound(alias.to_string()))
        }
    }

    fn sign(profile: PaymentProfile, secret: &secp256k1::SecretKey) -> SignedPaymentProfile {
        sign_profile(profile, secret).unwrap()
    }

    fn base_profile(
        alias: &str,
        methods: Vec<PaymentMethod>,
    ) -> (PaymentProfile, secp256k1::SecretKey) {
        let kp = generate_identity_keypair();
        let profile = PaymentProfile {
            alias: alias.to_string(),
            identity_pubkey: hex::encode(kp.public_key.serialize()),
            methods,
            updated_at: 1_700_000_000,
            expires_at: None,
            method_verifications: Vec::new(),
        };
        (profile, kp.secret_key)
    }

    fn lightning_method(addr: &str) -> PaymentMethod {
        PaymentMethod::Lightning {
            label: "Lightning Address".into(),
            lightning_address: Some(addr.into()),
            lnurl: None,
            bolt12: None,
            receiver_pubkey: None,
        }
    }

    fn onchain_method(addr: &str) -> PaymentMethod {
        PaymentMethod::Onchain {
            label: "Bitcoin".into(),
            network: BitcoinNetwork::Mainnet,
            address: addr.into(),
            pubkey_hint: None,
            descriptor_hint: None,
        }
    }

    fn ark_method(server: &str, pubkey: &str) -> PaymentMethod {
        PaymentMethod::Ark {
            label: "Ark".into(),
            server: server.into(),
            pubkey: pubkey.into(),
            vtxo_pointer: None,
            proof: None,
            expires_at: None,
        }
    }

    fn lightning_resolver(alias: &str, addr: &str) -> MockResolver {
        let (profile, secret) = base_profile(alias, vec![lightning_method(addr)]);
        MockResolver {
            signed: Some(sign(profile, &secret)),
        }
    }

    // 1–7: registered Lightning user → status ok with full payload.
    #[tokio::test]
    async fn registered_lightning_user_returns_ok_with_full_payload() {
        let resolver = lightning_resolver("rodrigo@satspath.dev", "rodrigo@getalby.com");
        let resp = quote_inner(
            &resolver,
            "rodrigo@satspath.dev",
            1000,
            Some(cheap_fees()),
            false,
        )
        .await;

        match resp {
            QuoteResponse::Ok {
                recipient,
                selected_method,
                fee_sats,
                eta,
                reason,
                qr,
                execution,
                wallet_hint,
            } => {
                // (2) alias, (3) verified true, (4) fingerprint
                assert_eq!(recipient.alias, "rodrigo@satspath.dev");
                assert!(recipient.verified);
                assert!(recipient.fingerprint.is_some());
                assert_eq!(recipient.fingerprint.as_ref().unwrap().len(), 8);
                // (5) PaymentMethod embedded unchanged
                assert!(matches!(selected_method, PaymentMethod::Lightning { .. }));
                // (6) fee_sats, eta, reason
                assert!(fee_sats.is_some());
                assert!(eta.is_some());
                assert!(!reason.is_empty());
                // (7) QR / payment payload present
                assert!(!qr.is_empty());
                assert_eq!(qr, "rodrigo@getalby.com"); // pointer (no fetch in this path)
            }
            other => panic!("expected Ok, got {}", other.status()),
        }
    }

    // 8–9: unknown user → not_registered with invite fields.
    #[tokio::test]
    async fn unknown_user_returns_not_registered_with_invite() {
        let resolver = MockResolver { signed: None };
        let resp = quote_inner(
            &resolver,
            "ghost@nowhere.dev",
            21_000,
            Some(cheap_fees()),
            false,
        )
        .await;
        match resp {
            QuoteResponse::NotRegistered { invite } => {
                assert!(!invite.alias_hash.is_empty());
                assert_eq!(invite.amount_sats, 21_000);
                assert!(invite.claim_url.contains("alias_hash="));
                assert!(invite.claim_url.contains("amount=21000"));
            }
            other => panic!("expected NotRegistered, got {}", other.status()),
        }
    }

    // 10: no usable route → no_route.
    #[tokio::test]
    async fn no_usable_route_returns_no_route() {
        // On-chain only, fees too high (35 > 20), no Lightning/Ark fallback.
        let (profile, secret) = base_profile(
            "carol@satspath.dev",
            vec![onchain_method("1BoatSLRHtKNngkdXEeobR76b53LETtpyT")],
        );
        let resolver = MockResolver {
            signed: Some(sign(profile, &secret)),
        };
        let resp = quote_inner(
            &resolver,
            "carol@satspath.dev",
            500_000,
            Some(expensive_fees()),
            false,
        )
        .await;
        assert_eq!(resp.status(), "no_route");
        assert!(matches!(resp, QuoteResponse::NoRoute { .. }));
    }

    // 11 & 12: invalid signature → invalid_signature, and never ok.
    #[tokio::test]
    async fn tampered_profile_returns_invalid_signature_never_ok() {
        let (profile, secret) = base_profile(
            "rodrigo@satspath.dev",
            vec![lightning_method("r@getalby.com")],
        );
        let mut signed = sign(profile, &secret);
        // Tamper after signing — signature no longer matches.
        signed.profile.alias = "evil@hacker.com".into();
        let resolver = MockResolver {
            signed: Some(signed),
        };
        let resp = quote_inner(
            &resolver,
            "evil@hacker.com",
            1000,
            Some(cheap_fees()),
            false,
        )
        .await;

        assert_ne!(
            resp.status(),
            "ok",
            "a tampered profile must never return ok"
        );
        match resp {
            QuoteResponse::InvalidSignature { recipient } => {
                assert!(!recipient.verified);
                assert!(recipient.fingerprint.is_some());
            }
            other => panic!("expected InvalidSignature, got {}", other.status()),
        }
    }

    // 13: on-chain QR is a BIP-21 URI.
    #[test]
    fn qr_payload_onchain_is_bip21() {
        let method = onchain_method("bc1qexampleaddr00000000000000000000000000");
        let qr = build_qr_payload(&method, 21_000).unwrap();
        assert_eq!(
            qr,
            "bitcoin:bc1qexampleaddr00000000000000000000000000?amount=0.00021000"
        );
    }

    // 14: Ark QR is a satspath:ark URI.
    #[test]
    fn qr_payload_ark_is_satspath_uri() {
        let method = ark_method(
            "https://ark.example.com",
            "02c0ded4d352532ee6d5d3fb69c7f08988c0a2b9f68a10fa79b2e769c123456789",
        );
        let qr = build_qr_payload(&method, 42).unwrap();
        assert!(qr.starts_with("satspath:ark?"));
        assert!(qr.contains("server=https%3A%2F%2Fark.example.com"));
        assert!(qr.contains("pubkey=02c0ded4"));
        assert!(qr.contains("amount=42"));
    }

    // 15: JSON serialization uses snake_case status values.
    #[tokio::test]
    async fn json_uses_snake_case_status_values() {
        let resolver = lightning_resolver("rodrigo@satspath.dev", "rodrigo@getalby.com");
        let ok = quote_inner(
            &resolver,
            "rodrigo@satspath.dev",
            1000,
            Some(cheap_fees()),
            false,
        )
        .await;
        let json = serde_json::to_string(&ok).unwrap();
        assert!(json.contains("\"status\":\"ok\""), "json: {json}");

        let nr = quote_inner(
            &MockResolver { signed: None },
            "x@y.z",
            1,
            Some(cheap_fees()),
            false,
        )
        .await;
        assert!(serde_json::to_string(&nr)
            .unwrap()
            .contains("\"status\":\"not_registered\""));

        let (p, s) = base_profile(
            "c@d.e",
            vec![onchain_method("1BoatSLRHtKNngkdXEeobR76b53LETtpyT")],
        );
        let route_resolver = MockResolver {
            signed: Some(sign(p, &s)),
        };
        let nroute = quote_inner(
            &route_resolver,
            "c@d.e",
            500_000,
            Some(expensive_fees()),
            false,
        )
        .await;
        assert!(serde_json::to_string(&nroute)
            .unwrap()
            .contains("\"status\":\"no_route\""));

        let (p2, s2) = base_profile("r@s.t", vec![lightning_method("r@getalby.com")]);
        let mut tampered = sign(p2, &s2);
        tampered.profile.alias = "evil@x.y".into();
        let inv_resolver = MockResolver {
            signed: Some(tampered),
        };
        let inv = quote_inner(&inv_resolver, "evil@x.y", 1000, Some(cheap_fees()), false).await;
        assert!(serde_json::to_string(&inv)
            .unwrap()
            .contains("\"status\":\"invalid_signature\""));
    }

    // Success JSON embeds PaymentMethod with its `type` tag, unchanged.
    #[tokio::test]
    async fn ok_json_embeds_selected_method_with_type_tag() {
        let resolver = lightning_resolver("rodrigo@satspath.dev", "rodrigo@getalby.com");
        let ok = quote_inner(
            &resolver,
            "rodrigo@satspath.dev",
            1000,
            Some(cheap_fees()),
            false,
        )
        .await;
        let v: serde_json::Value = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["selected_method"]["type"], "Lightning");
        assert_eq!(
            v["selected_method"]["lightning_address"],
            "rodrigo@getalby.com"
        );
        assert_eq!(v["recipient"]["verified"], true);
    }
}
