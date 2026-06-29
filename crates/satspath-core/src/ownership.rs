//! Payment-method ownership proofs and client-side validation.
//!
//! SatsPath profiles list payment *claims*: "pay me here". A claim alone says
//! nothing about whether the profile owner actually controls that pointer. This
//! module lets each method carry a verifiable **ownership proof** so a resolver
//! can distinguish an unverified claim from a cryptographically (or domain-)
//! attested one — entirely client-side, with no funds, signing of spends, or
//! private wallet material involved.
//!
//! ## Trust tiers
//!
//! | Proof type                  | What it proves                                  | Tier          |
//! |-----------------------------|-------------------------------------------------|---------------|
//! | `OnchainAddressSignature`   | The address key signed an identity-bound message| Cryptographic |
//! | `ArkPubkeySignature`        | The Ark key signed an identity-bound message    | Cryptographic |
//! | `DomainWellKnown`           | A domain served identity-bound content          | DomainControl |
//! | `LightningAddressChallenge` | The LN-address domain served identity-bound data| DomainControl |
//! | `ManualAttestation`         | The identity self-asserts (no third-party proof)| SelfAsserted  |
//!
//! ## Security invariants
//!
//! * Proofs only ever carry **public** material (signatures, pubkeys, URLs,
//!   nonces, hashes). [`validate_ownership_proof`] rejects anything resembling
//!   private material.
//! * A proof is bound to a method via [`PaymentMethod::ownership_descriptor`]
//!   and to the profile via `identity_pubkey`, so it cannot be replayed onto a
//!   different method or identity.
//! * Signing keys (the address/Ark keys) are borrowed transiently when *building*
//!   a proof and are never stored. Only the resulting signature + public key are
//!   retained.
//! * Verification is offline. Network-fetched content (well-known bodies) is
//!   passed in by the caller; this module never performs I/O.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use bitcoin::{Address, CompressedPublicKey, Network, XOnlyPublicKey};

use crate::crypto::verify_message_signature;
use crate::errors::{Result, SatsPathError};
use crate::pointer::BitcoinNetwork;
use crate::profile::PaymentMethod;
use crate::validation::assert_no_private_material;

/// The kind of evidence backing an ownership proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofType {
    DomainWellKnown,
    LightningAddressChallenge,
    OnchainAddressSignature,
    ArkPubkeySignature,
    ManualAttestation,
}

/// How much weight a verified proof carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustTier {
    /// A key with spending authority signed an identity-bound challenge.
    Cryptographic,
    /// A domain under the claimant's control served identity-bound content.
    DomainControl,
    /// The identity asserts ownership itself, with no independent evidence.
    SelfAsserted,
}

impl ProofType {
    pub fn trust_tier(self) -> TrustTier {
        match self {
            ProofType::OnchainAddressSignature | ProofType::ArkPubkeySignature => {
                TrustTier::Cryptographic
            }
            ProofType::DomainWellKnown | ProofType::LightningAddressChallenge => {
                TrustTier::DomainControl
            }
            ProofType::ManualAttestation => TrustTier::SelfAsserted,
        }
    }
}

/// The concrete evidence carried by a proof. Public material only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "proof")]
pub enum OwnershipProof {
    /// An ECDSA signature over an identity-bound challenge message.
    /// `pubkey` is the signing public key (required for signature-based proofs).
    MessageSignature {
        message: String,
        signature: String,
        #[serde(default)]
        pubkey: Option<String>,
    },
    /// A commitment to content served at a well-known URL.
    WellKnownChallenge {
        url: String,
        nonce: String,
        expected_body_hash: String,
    },
}

/// Whether a method is an unverified claim or carries a verified ownership proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum VerificationStatus {
    /// A bare claim. The profile owner asserts this pointer but offered no proof.
    Unverified,
    /// A proof was supplied. Validity is re-checked at resolve time; this stored
    /// form is not trusted blindly.
    Verified {
        proof_type: ProofType,
        verified_at: i64,
        #[serde(default)]
        expires_at: Option<i64>,
        proof: OwnershipProof,
    },
}

impl VerificationStatus {
    pub fn is_verified(&self) -> bool {
        matches!(self, VerificationStatus::Verified { .. })
    }

    /// True only if verified *and* not past `expires_at` at `now`.
    pub fn is_currently_valid(&self, now: i64) -> bool {
        match self {
            VerificationStatus::Unverified => false,
            VerificationStatus::Verified { expires_at, .. } => {
                expires_at.map(|exp| now < exp).unwrap_or(true)
            }
        }
    }
}

/// An ownership proof bound to one payment method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodVerification {
    /// Must equal the target method's [`PaymentMethod::ownership_descriptor`].
    pub method_descriptor: String,
    pub status: VerificationStatus,
}

// ─── Challenge message ────────────────────────────────────────────────────────

/// The exact message a key must sign to prove it controls a method on behalf of
/// an identity. Domain-separated from profile signing, and bound to the identity,
/// the specific method, and a timestamp so it cannot be replayed.
pub fn ownership_challenge_message(
    identity_pubkey: &str,
    method_descriptor: &str,
    issued_at: i64,
) -> String {
    format!(
        "SatsPath Ownership Proof v1\nidentity={identity_pubkey}\nmethod={method_descriptor}\nissued_at={issued_at}"
    )
}

// ─── Structural validation (no private material, well-formed fields) ──────────

pub fn validate_ownership_proof(proof: &OwnershipProof) -> Result<()> {
    match proof {
        OwnershipProof::MessageSignature {
            message,
            signature,
            pubkey,
        } => {
            assert_no_private_material(message)?;
            hex::decode(signature)
                .map_err(|e| SatsPathError::OwnershipProofInvalid(format!("signature hex: {e}")))?;
            if let Some(pk) = pubkey {
                crate::validation::validate_compressed_pubkey(pk)?;
            }
            Ok(())
        }
        OwnershipProof::WellKnownChallenge {
            url,
            nonce,
            expected_body_hash,
        } => {
            let parsed = url::Url::parse(url)
                .map_err(|e| SatsPathError::OwnershipProofInvalid(format!("url: {e}")))?;
            if parsed.scheme() != "https" {
                return Err(SatsPathError::OwnershipProofInvalid(
                    "well-known proof URL must be https".into(),
                ));
            }
            if nonce.trim().is_empty() {
                return Err(SatsPathError::OwnershipProofInvalid("empty nonce".into()));
            }
            // 32-byte SHA-256 digest, hex-encoded.
            let bytes = hex::decode(expected_body_hash)
                .map_err(|e| SatsPathError::OwnershipProofInvalid(format!("body hash hex: {e}")))?;
            if bytes.len() != 32 {
                return Err(SatsPathError::OwnershipProofInvalid(
                    "expected_body_hash must be a 32-byte SHA-256 digest".into(),
                ));
            }
            assert_no_private_material(nonce)?;
            Ok(())
        }
    }
}

pub fn validate_method_verification(verification: &MethodVerification) -> Result<()> {
    assert_no_private_material(&verification.method_descriptor)?;
    if let VerificationStatus::Verified {
        proof,
        expires_at,
        verified_at,
        ..
    } = &verification.status
    {
        if let Some(exp) = expires_at {
            if *exp <= *verified_at {
                return Err(SatsPathError::OwnershipProofInvalid(
                    "expires_at must be after verified_at".into(),
                ));
            }
        }
        validate_ownership_proof(proof)?;
    }
    Ok(())
}

// ─── Building proofs ──────────────────────────────────────────────────────────

/// Build a cryptographic ownership proof for an on-chain or Ark method by signing
/// the identity-bound challenge with the method's own key.
///
/// `signing_secret_key` is the secret key controlling the address/Ark pubkey. It
/// is used only to produce the signature and is **never** stored — only the
/// resulting signature and the corresponding public key are retained.
pub fn build_signature_attestation(
    method: &PaymentMethod,
    identity_pubkey: &str,
    proof_type: ProofType,
    signing_secret_key: &secp256k1::SecretKey,
    verified_at: i64,
    expires_at: Option<i64>,
) -> Result<MethodVerification> {
    if !matches!(
        proof_type,
        ProofType::OnchainAddressSignature | ProofType::ArkPubkeySignature
    ) {
        return Err(SatsPathError::OwnershipProofUnsupported(
            "build_signature_attestation only handles signature proof types".into(),
        ));
    }
    let descriptor = method.ownership_descriptor();
    let message = ownership_challenge_message(identity_pubkey, &descriptor, verified_at);

    let secp = secp256k1::Secp256k1::new();
    let signing_pubkey = secp256k1::PublicKey::from_secret_key(&secp, signing_secret_key);
    let signature = crate::crypto::sign_message(&message, signing_secret_key);

    let verification = MethodVerification {
        method_descriptor: descriptor,
        status: VerificationStatus::Verified {
            proof_type,
            verified_at,
            expires_at,
            proof: OwnershipProof::MessageSignature {
                message,
                signature,
                pubkey: Some(hex::encode(signing_pubkey.serialize())),
            },
        },
    };
    // Verify what we just built so we never emit an attestation that fails its
    // own checks (e.g. on-chain key that does not derive the claimed address).
    verify_method_verification(method, identity_pubkey, &verification, verified_at, None)?;
    Ok(verification)
}

/// Build a self-asserted attestation signed by the identity key itself. This is
/// the weakest tier: it proves only that the profile owner *claims* ownership.
pub fn build_manual_attestation(
    method: &PaymentMethod,
    identity_pubkey: &str,
    identity_secret_key: &secp256k1::SecretKey,
    verified_at: i64,
    expires_at: Option<i64>,
) -> Result<MethodVerification> {
    let descriptor = method.ownership_descriptor();
    let message = ownership_challenge_message(identity_pubkey, &descriptor, verified_at);
    let signature = crate::crypto::sign_message(&message, identity_secret_key);
    Ok(MethodVerification {
        method_descriptor: descriptor,
        status: VerificationStatus::Verified {
            proof_type: ProofType::ManualAttestation,
            verified_at,
            expires_at,
            proof: OwnershipProof::MessageSignature {
                message,
                signature,
                pubkey: Some(identity_pubkey.to_string()),
            },
        },
    })
}

// ─── Verification (client-side) ───────────────────────────────────────────────

/// Re-verify a stored [`MethodVerification`] against its method and identity.
///
/// Returns the [`TrustTier`] on success. `well_known_body` is required for
/// domain-control proof types and must be the content the caller fetched from the
/// proof URL; it is ignored for signature-based and manual proofs.
///
/// This is the function a resolver calls — never trust a stored `Verified`
/// status without running it.
pub fn verify_method_verification(
    method: &PaymentMethod,
    identity_pubkey: &str,
    verification: &MethodVerification,
    now: i64,
    well_known_body: Option<&str>,
) -> Result<TrustTier> {
    // 1. The proof must be bound to *this* method.
    let expected = method.ownership_descriptor();
    if verification.method_descriptor != expected {
        return Err(SatsPathError::OwnershipProofInvalid(format!(
            "descriptor mismatch: proof binds '{}', method is '{}'",
            verification.method_descriptor, expected
        )));
    }

    let (proof_type, verified_at, expires_at, proof) = match &verification.status {
        VerificationStatus::Unverified => {
            return Err(SatsPathError::OwnershipProofInvalid(
                "method is Unverified — nothing to verify".into(),
            ))
        }
        VerificationStatus::Verified {
            proof_type,
            verified_at,
            expires_at,
            proof,
        } => (proof_type, *verified_at, *expires_at, proof),
    };

    // 2. Expiry.
    if let Some(exp) = expires_at {
        if now >= exp {
            return Err(SatsPathError::OwnershipProofExpired);
        }
    }

    // 3. Dispatch by proof type.
    let challenge = ownership_challenge_message(identity_pubkey, &expected, verified_at);
    match proof_type {
        ProofType::OnchainAddressSignature => {
            verify_onchain_signature(method, &challenge, proof)?;
        }
        ProofType::ArkPubkeySignature => {
            verify_ark_signature(method, &challenge, proof)?;
        }
        ProofType::ManualAttestation => {
            verify_manual(identity_pubkey, &challenge, proof)?;
        }
        ProofType::DomainWellKnown => {
            let body = well_known_body.ok_or_else(|| {
                SatsPathError::OwnershipProofInvalid(
                    "well-known body required to verify DomainWellKnown proof".into(),
                )
            })?;
            verify_well_known(identity_pubkey, None, proof, body)?;
        }
        ProofType::LightningAddressChallenge => {
            let body = well_known_body.ok_or_else(|| {
                SatsPathError::OwnershipProofInvalid(
                    "well-known body required to verify LightningAddressChallenge proof".into(),
                )
            })?;
            let domain = lightning_method_domain(method)?;
            verify_well_known(identity_pubkey, Some(&domain), proof, body)?;
        }
    }

    Ok(proof_type.trust_tier())
}

fn expect_message_signature(proof: &OwnershipProof) -> Result<(&str, &str, &str)> {
    match proof {
        OwnershipProof::MessageSignature {
            message,
            signature,
            pubkey,
        } => {
            let pk = pubkey.as_deref().ok_or_else(|| {
                SatsPathError::OwnershipProofInvalid("signature proof missing pubkey".into())
            })?;
            Ok((message, signature, pk))
        }
        OwnershipProof::WellKnownChallenge { .. } => Err(SatsPathError::OwnershipProofInvalid(
            "expected a MessageSignature proof".into(),
        )),
    }
}

fn verify_onchain_signature(
    method: &PaymentMethod,
    challenge: &str,
    proof: &OwnershipProof,
) -> Result<()> {
    let (message, signature, pubkey) = expect_message_signature(proof)?;
    if message != challenge {
        return Err(SatsPathError::OwnershipProofInvalid(
            "signed message does not match the expected challenge".into(),
        ));
    }
    let PaymentMethod::Onchain {
        address, network, ..
    } = method
    else {
        return Err(SatsPathError::OwnershipProofInvalid(
            "OnchainAddressSignature applied to a non-onchain method".into(),
        ));
    };
    // The key must (a) have signed the challenge and (b) actually derive the
    // claimed address — otherwise anyone could sign with an unrelated key.
    if !verify_message_signature(message, signature, pubkey)? {
        return Err(SatsPathError::OwnershipProofInvalid(
            "on-chain signature does not verify".into(),
        ));
    }
    if !pubkey_controls_address(pubkey, address, *network)? {
        return Err(SatsPathError::OwnershipProofInvalid(
            "signing key does not derive the claimed address".into(),
        ));
    }
    Ok(())
}

fn verify_ark_signature(
    method: &PaymentMethod,
    challenge: &str,
    proof: &OwnershipProof,
) -> Result<()> {
    let (message, signature, pubkey) = expect_message_signature(proof)?;
    if message != challenge {
        return Err(SatsPathError::OwnershipProofInvalid(
            "signed message does not match the expected challenge".into(),
        ));
    }
    let PaymentMethod::Ark {
        pubkey: ark_pubkey, ..
    } = method
    else {
        return Err(SatsPathError::OwnershipProofInvalid(
            "ArkPubkeySignature applied to a non-Ark method".into(),
        ));
    };
    if pubkey != ark_pubkey {
        return Err(SatsPathError::OwnershipProofInvalid(
            "signing key is not the Ark method's pubkey".into(),
        ));
    }
    if !verify_message_signature(message, signature, pubkey)? {
        return Err(SatsPathError::OwnershipProofInvalid(
            "Ark signature does not verify".into(),
        ));
    }
    Ok(())
}

fn verify_manual(identity_pubkey: &str, challenge: &str, proof: &OwnershipProof) -> Result<()> {
    let (message, signature, pubkey) = expect_message_signature(proof)?;
    if message != challenge {
        return Err(SatsPathError::OwnershipProofInvalid(
            "self-attestation message does not match the expected challenge".into(),
        ));
    }
    // A manual attestation must be signed by the identity itself.
    if pubkey != identity_pubkey {
        return Err(SatsPathError::OwnershipProofInvalid(
            "manual attestation must be signed by the identity key".into(),
        ));
    }
    if !verify_message_signature(message, signature, identity_pubkey)? {
        return Err(SatsPathError::OwnershipProofInvalid(
            "self-attestation signature does not verify".into(),
        ));
    }
    Ok(())
}

fn verify_well_known(
    identity_pubkey: &str,
    expected_domain: Option<&str>,
    proof: &OwnershipProof,
    fetched_body: &str,
) -> Result<()> {
    let OwnershipProof::WellKnownChallenge {
        url,
        nonce,
        expected_body_hash,
    } = proof
    else {
        return Err(SatsPathError::OwnershipProofInvalid(
            "expected a WellKnownChallenge proof".into(),
        ));
    };

    let parsed = url::Url::parse(url)
        .map_err(|e| SatsPathError::OwnershipProofInvalid(format!("url: {e}")))?;
    if parsed.scheme() != "https" {
        return Err(SatsPathError::OwnershipProofInvalid(
            "well-known proof URL must be https".into(),
        ));
    }
    if let Some(domain) = expected_domain {
        let host = parsed.host_str().unwrap_or("");
        if !host.eq_ignore_ascii_case(domain) {
            return Err(SatsPathError::OwnershipProofInvalid(format!(
                "well-known host '{host}' does not match expected domain '{domain}'"
            )));
        }
    }

    // The fetched content must hash to the committed value...
    let digest = hex::encode(Sha256::digest(fetched_body.as_bytes()));
    if &digest != expected_body_hash {
        return Err(SatsPathError::OwnershipProofInvalid(
            "fetched body hash does not match the committed expected_body_hash".into(),
        ));
    }
    // ...and bind the served content to this identity and challenge nonce, so a
    // generic/static file cannot be passed off as a proof.
    if !fetched_body.contains(nonce) {
        return Err(SatsPathError::OwnershipProofInvalid(
            "fetched body does not contain the challenge nonce".into(),
        ));
    }
    if !fetched_body.contains(identity_pubkey) {
        return Err(SatsPathError::OwnershipProofInvalid(
            "fetched body does not contain the identity pubkey".into(),
        ));
    }
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn lightning_method_domain(method: &PaymentMethod) -> Result<String> {
    match method {
        PaymentMethod::Lightning {
            lightning_address: Some(addr),
            ..
        } => addr
            .split_once('@')
            .map(|(_, d)| d.trim().to_ascii_lowercase())
            .ok_or_else(|| {
                SatsPathError::OwnershipProofInvalid("malformed Lightning Address".into())
            }),
        PaymentMethod::Lightning {
            lnurl: Some(url), ..
        } => url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
            .ok_or_else(|| SatsPathError::OwnershipProofInvalid("malformed LNURL".into())),
        _ => Err(SatsPathError::OwnershipProofInvalid(
            "LightningAddressChallenge requires a Lightning Address or LNURL method".into(),
        )),
    }
}

fn map_network(network: BitcoinNetwork) -> Network {
    match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    }
}

/// True if `pubkey_hex` (a 33-byte compressed secp256k1 key) derives `address`
/// as either a P2WPKH (bech32 v0) or P2TR key-path (bech32m v1) address on the
/// given network. Legacy P2PKH/P2SH are intentionally unsupported — we fail
/// closed rather than guess.
pub fn pubkey_controls_address(
    pubkey_hex: &str,
    address: &str,
    network: BitcoinNetwork,
) -> Result<bool> {
    let net = map_network(network);
    let bytes =
        hex::decode(pubkey_hex).map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;

    // P2WPKH
    if let Ok(cpk) = CompressedPublicKey::from_slice(&bytes) {
        let p2wpkh = Address::p2wpkh(&cpk, net);
        if p2wpkh.to_string() == address {
            return Ok(true);
        }
    }

    // P2TR (key-path, no script tree)
    if bytes.len() == 33 {
        if let Ok(xonly) = XOnlyPublicKey::from_slice(&bytes[1..]) {
            let secp = bitcoin::secp256k1::Secp256k1::verification_only();
            let p2tr = Address::p2tr(&secp, xonly, None, net);
            if p2tr.to_string() == address {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Resolve the stored verification (if any) for a given method within a profile,
/// returning [`VerificationStatus::Unverified`] when no proof is attached.
pub fn stored_status_for_method<'a>(
    verifications: &'a [MethodVerification],
    method: &PaymentMethod,
) -> &'a VerificationStatus {
    let descriptor = method.ownership_descriptor();
    verifications
        .iter()
        .find(|v| v.method_descriptor == descriptor)
        .map(|v| &v.status)
        .unwrap_or(&VerificationStatus::Unverified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_identity_keypair, sign_message};
    use crate::profile::PaymentMethod;
    use bitcoin::{Address, CompressedPublicKey, Network, XOnlyPublicKey};
    use secp256k1::{Secp256k1, SecretKey};

    const NOW: i64 = 1_700_000_000;

    struct Key {
        secret: SecretKey,
        pubkey_hex: String,
        pubkey_bytes: Vec<u8>,
    }

    fn key() -> Key {
        let kp = generate_identity_keypair();
        Key {
            secret: kp.secret_key,
            pubkey_hex: hex::encode(kp.public_key.serialize()),
            pubkey_bytes: kp.public_key.serialize().to_vec(),
        }
    }

    fn p2wpkh(k: &Key, net: Network) -> String {
        let cpk = CompressedPublicKey::from_slice(&k.pubkey_bytes).unwrap();
        Address::p2wpkh(&cpk, net).to_string()
    }

    fn p2tr(k: &Key, net: Network) -> String {
        let secp = Secp256k1::new();
        let xonly = XOnlyPublicKey::from_slice(&k.pubkey_bytes[1..]).unwrap();
        Address::p2tr(&secp, xonly, None, net).to_string()
    }

    fn onchain_method(address: &str) -> PaymentMethod {
        PaymentMethod::Onchain {
            label: "BTC".into(),
            network: BitcoinNetwork::Mainnet,
            address: address.into(),
            pubkey_hint: None,
            descriptor_hint: None,
        }
    }

    fn ark_method(pubkey_hex: &str) -> PaymentMethod {
        PaymentMethod::Ark {
            label: "Ark".into(),
            server: "https://ark.example.com".into(),
            pubkey: pubkey_hex.into(),
            vtxo_pointer: None,
        }
    }

    fn ln_method(addr: &str) -> PaymentMethod {
        PaymentMethod::Lightning {
            label: "LN".into(),
            lightning_address: Some(addr.into()),
            lnurl: None,
            bolt12: None,
            receiver_pubkey: None,
        }
    }

    // ── pubkey ↔ address derivation ───────────────────────────────────────────

    #[test]
    fn pubkey_controls_p2wpkh_and_p2tr() {
        let k = key();
        let wpkh = p2wpkh(&k, Network::Bitcoin);
        let tr = p2tr(&k, Network::Bitcoin);
        assert!(pubkey_controls_address(&k.pubkey_hex, &wpkh, BitcoinNetwork::Mainnet).unwrap());
        assert!(pubkey_controls_address(&k.pubkey_hex, &tr, BitcoinNetwork::Mainnet).unwrap());
    }

    #[test]
    fn pubkey_does_not_control_unrelated_address() {
        let k = key();
        let other = key();
        let other_addr = p2wpkh(&other, Network::Bitcoin);
        assert!(
            !pubkey_controls_address(&k.pubkey_hex, &other_addr, BitcoinNetwork::Mainnet).unwrap()
        );
    }

    // ── On-chain signature proof ──────────────────────────────────────────────

    #[test]
    fn onchain_signature_proof_verifies_as_cryptographic() {
        let identity = key();
        let addr_key = key();
        let address = p2wpkh(&addr_key, Network::Bitcoin);
        let method = onchain_method(&address);

        let verification = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::OnchainAddressSignature,
            &addr_key.secret,
            NOW,
            None,
        )
        .unwrap();

        let tier =
            verify_method_verification(&method, &identity.pubkey_hex, &verification, NOW, None)
                .unwrap();
        assert_eq!(tier, TrustTier::Cryptographic);
    }

    #[test]
    fn onchain_proof_with_key_not_controlling_address_rejected() {
        let identity = key();
        let addr_key = key();
        let wrong_key = key();
        // Address belongs to addr_key, but we sign/claim with wrong_key.
        let address = p2wpkh(&addr_key, Network::Bitcoin);
        let method = onchain_method(&address);

        // build_signature_attestation self-verifies, so it must refuse here.
        let built = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::OnchainAddressSignature,
            &wrong_key.secret,
            NOW,
            None,
        );
        assert!(
            built.is_err(),
            "must not emit a proof for a non-controlling key"
        );
    }

    #[test]
    fn onchain_proof_tampered_message_rejected() {
        let identity = key();
        let addr_key = key();
        let address = p2wpkh(&addr_key, Network::Bitcoin);
        let method = onchain_method(&address);

        let mut verification = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::OnchainAddressSignature,
            &addr_key.secret,
            NOW,
            None,
        )
        .unwrap();

        if let VerificationStatus::Verified {
            proof: OwnershipProof::MessageSignature { message, .. },
            ..
        } = &mut verification.status
        {
            *message = "SatsPath Ownership Proof v1\nidentity=evil".into();
        }
        assert!(verify_method_verification(
            &method,
            &identity.pubkey_hex,
            &verification,
            NOW,
            None
        )
        .is_err());
    }

    // ── Ark signature proof ───────────────────────────────────────────────────

    #[test]
    fn ark_signature_proof_verifies() {
        let identity = key();
        let ark_key = key();
        let method = ark_method(&ark_key.pubkey_hex);

        let verification = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::ArkPubkeySignature,
            &ark_key.secret,
            NOW,
            None,
        )
        .unwrap();

        let tier =
            verify_method_verification(&method, &identity.pubkey_hex, &verification, NOW, None)
                .unwrap();
        assert_eq!(tier, TrustTier::Cryptographic);
    }

    #[test]
    fn ark_proof_with_wrong_pubkey_rejected() {
        let identity = key();
        let ark_key = key();
        let other = key();
        // Method declares ark_key's pubkey, but proof is built with `other`.
        let method = ark_method(&ark_key.pubkey_hex);
        let built = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::ArkPubkeySignature,
            &other.secret,
            NOW,
            None,
        );
        assert!(built.is_err());
    }

    // ── Manual attestation ────────────────────────────────────────────────────

    #[test]
    fn manual_attestation_is_self_asserted() {
        let identity = key();
        let method = ln_method("alice@example.com");
        let verification =
            build_manual_attestation(&method, &identity.pubkey_hex, &identity.secret, NOW, None)
                .unwrap();
        let tier =
            verify_method_verification(&method, &identity.pubkey_hex, &verification, NOW, None)
                .unwrap();
        assert_eq!(tier, TrustTier::SelfAsserted);
    }

    #[test]
    fn manual_attestation_signed_by_other_key_rejected() {
        let identity = key();
        let imposter = key();
        let method = ln_method("alice@example.com");
        // Signed by imposter but claims to be identity's self-attestation.
        let descriptor = method.ownership_descriptor();
        let message = ownership_challenge_message(&identity.pubkey_hex, &descriptor, NOW);
        let signature = sign_message(&message, &imposter.secret);
        let verification = MethodVerification {
            method_descriptor: descriptor,
            status: VerificationStatus::Verified {
                proof_type: ProofType::ManualAttestation,
                verified_at: NOW,
                expires_at: None,
                proof: OwnershipProof::MessageSignature {
                    message,
                    signature,
                    pubkey: Some(imposter.pubkey_hex.clone()),
                },
            },
        };
        assert!(verify_method_verification(
            &method,
            &identity.pubkey_hex,
            &verification,
            NOW,
            None
        )
        .is_err());
    }

    // ── Well-known / domain control ───────────────────────────────────────────

    fn well_known(url: &str, nonce: &str, body: &str, proof_type: ProofType) -> MethodVerification {
        let hash = hex::encode(Sha256::digest(body.as_bytes()));
        MethodVerification {
            method_descriptor: String::new(), // set by caller
            status: VerificationStatus::Verified {
                proof_type,
                verified_at: NOW,
                expires_at: None,
                proof: OwnershipProof::WellKnownChallenge {
                    url: url.into(),
                    nonce: nonce.into(),
                    expected_body_hash: hash,
                },
            },
        }
    }

    #[test]
    fn domain_well_known_proof_verifies() {
        let identity = key();
        let method = onchain_method(&p2wpkh(&key(), Network::Bitcoin));
        let nonce = "challenge-nonce-123";
        let body = format!(
            "satspath-proof\nidentity={}\nnonce={nonce}",
            identity.pubkey_hex
        );
        let mut v = well_known(
            "https://example.com/.well-known/satspath",
            nonce,
            &body,
            ProofType::DomainWellKnown,
        );
        v.method_descriptor = method.ownership_descriptor();

        let tier = verify_method_verification(&method, &identity.pubkey_hex, &v, NOW, Some(&body))
            .unwrap();
        assert_eq!(tier, TrustTier::DomainControl);
    }

    #[test]
    fn well_known_wrong_body_hash_rejected() {
        let identity = key();
        let method = onchain_method(&p2wpkh(&key(), Network::Bitcoin));
        let nonce = "n";
        let body = format!("identity={} nonce={nonce}", identity.pubkey_hex);
        let mut v = well_known(
            "https://example.com/p",
            nonce,
            &body,
            ProofType::DomainWellKnown,
        );
        v.method_descriptor = method.ownership_descriptor();
        // Serve a DIFFERENT body than the one committed.
        let tampered = format!("{body} tampered");
        assert!(verify_method_verification(
            &method,
            &identity.pubkey_hex,
            &v,
            NOW,
            Some(&tampered)
        )
        .is_err());
    }

    #[test]
    fn well_known_missing_identity_binding_rejected() {
        let identity = key();
        let method = onchain_method(&p2wpkh(&key(), Network::Bitcoin));
        let nonce = "n";
        let body = format!("nonce={nonce} but no identity pubkey here");
        let mut v = well_known(
            "https://example.com/p",
            nonce,
            &body,
            ProofType::DomainWellKnown,
        );
        v.method_descriptor = method.ownership_descriptor();
        assert!(
            verify_method_verification(&method, &identity.pubkey_hex, &v, NOW, Some(&body))
                .is_err()
        );
    }

    #[test]
    fn lightning_challenge_requires_matching_host() {
        let identity = key();
        let method = ln_method("alice@example.com");
        let nonce = "n";
        let body = format!("identity={} nonce={nonce}", identity.pubkey_hex);
        // URL host is attacker.com, but the LN address domain is example.com.
        let mut v = well_known(
            "https://attacker.com/.well-known/satspath",
            nonce,
            &body,
            ProofType::LightningAddressChallenge,
        );
        v.method_descriptor = method.ownership_descriptor();
        assert!(
            verify_method_verification(&method, &identity.pubkey_hex, &v, NOW, Some(&body))
                .is_err()
        );
    }

    #[test]
    fn lightning_challenge_matching_host_verifies() {
        let identity = key();
        let method = ln_method("alice@example.com");
        let nonce = "n";
        let body = format!("identity={} nonce={nonce}", identity.pubkey_hex);
        let mut v = well_known(
            "https://example.com/.well-known/satspath",
            nonce,
            &body,
            ProofType::LightningAddressChallenge,
        );
        v.method_descriptor = method.ownership_descriptor();
        let tier = verify_method_verification(&method, &identity.pubkey_hex, &v, NOW, Some(&body))
            .unwrap();
        assert_eq!(tier, TrustTier::DomainControl);
    }

    // ── Replay / binding / expiry ─────────────────────────────────────────────

    #[test]
    fn proof_cannot_be_replayed_onto_different_method() {
        let identity = key();
        let addr_key = key();
        let address = p2wpkh(&addr_key, Network::Bitcoin);
        let method = onchain_method(&address);
        let verification = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::OnchainAddressSignature,
            &addr_key.secret,
            NOW,
            None,
        )
        .unwrap();

        // A different on-chain method (different address) must reject this proof.
        let other_method = onchain_method(&p2wpkh(&key(), Network::Bitcoin));
        assert!(verify_method_verification(
            &other_method,
            &identity.pubkey_hex,
            &verification,
            NOW,
            None
        )
        .is_err());
    }

    #[test]
    fn expired_proof_rejected() {
        let identity = key();
        let addr_key = key();
        let address = p2wpkh(&addr_key, Network::Bitcoin);
        let method = onchain_method(&address);
        let verification = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::OnchainAddressSignature,
            &addr_key.secret,
            NOW,
            Some(NOW + 100),
        )
        .unwrap();

        // Valid before expiry, rejected at/after.
        assert!(verify_method_verification(
            &method,
            &identity.pubkey_hex,
            &verification,
            NOW + 50,
            None
        )
        .is_ok());
        let err = verify_method_verification(
            &method,
            &identity.pubkey_hex,
            &verification,
            NOW + 100,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, SatsPathError::OwnershipProofExpired));
    }

    #[test]
    fn unverified_status_has_no_proof_to_check() {
        let method = ln_method("alice@example.com");
        let identity = key();
        let v = MethodVerification {
            method_descriptor: method.ownership_descriptor(),
            status: VerificationStatus::Unverified,
        };
        assert!(verify_method_verification(&method, &identity.pubkey_hex, &v, NOW, None).is_err());
        assert!(!v.status.is_verified());
        assert!(!v.status.is_currently_valid(NOW));
    }

    // ── Structural validation ─────────────────────────────────────────────────

    #[test]
    fn validate_rejects_private_material_and_malformed_fields() {
        // Private material in a message.
        let bad = OwnershipProof::MessageSignature {
            message: "here is my xprv9s21Z secret".into(),
            signature: "00".into(),
            pubkey: None,
        };
        assert!(validate_ownership_proof(&bad).is_err());

        // Non-https well-known URL.
        let bad_url = OwnershipProof::WellKnownChallenge {
            url: "http://example.com/p".into(),
            nonce: "n".into(),
            expected_body_hash: hex::encode([0u8; 32]),
        };
        assert!(validate_ownership_proof(&bad_url).is_err());

        // Wrong-length body hash.
        let bad_hash = OwnershipProof::WellKnownChallenge {
            url: "https://example.com/p".into(),
            nonce: "n".into(),
            expected_body_hash: "abcd".into(),
        };
        assert!(validate_ownership_proof(&bad_hash).is_err());
    }

    #[test]
    fn stored_status_defaults_to_unverified() {
        let method = ln_method("alice@example.com");
        let status = stored_status_for_method(&[], &method);
        assert!(matches!(status, VerificationStatus::Unverified));
    }

    // ── Serde / tamper-evidence through the identity signature ────────────────

    #[test]
    fn identity_signature_commits_to_verifications() {
        use crate::crypto::{sign_profile, verify_signed_profile};
        use crate::profile::PaymentProfile;

        let identity = key();
        let addr_key = key();
        let address = p2wpkh(&addr_key, Network::Bitcoin);
        let method = onchain_method(&address);
        let verification = build_signature_attestation(
            &method,
            &identity.pubkey_hex,
            ProofType::OnchainAddressSignature,
            &addr_key.secret,
            NOW,
            None,
        )
        .unwrap();

        let profile = PaymentProfile {
            alias: "alice@example.com".into(),
            identity_pubkey: identity.pubkey_hex.clone(),
            methods: vec![method],
            updated_at: NOW,
            expires_at: None,
            method_verifications: vec![verification],
        };
        let mut signed = sign_profile(profile, &identity.secret).unwrap();
        assert!(verify_signed_profile(&signed).unwrap());

        // Tampering with the attestation after signing must break the signature.
        if let VerificationStatus::Verified { verified_at, .. } =
            &mut signed.profile.method_verifications[0].status
        {
            *verified_at += 1;
        }
        assert!(!verify_signed_profile(&signed).unwrap());
    }

    #[test]
    fn legacy_profile_without_verifications_deserializes_and_omits_field() {
        use crate::profile::PaymentProfile;

        // A profile authored before ownership proofs existed (no field present).
        let legacy = r#"{
            "alias": "bob@example.com",
            "identity_pubkey": "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            "methods": [],
            "updated_at": 1700000000
        }"#;
        let profile: PaymentProfile = serde_json::from_str(legacy).unwrap();
        assert!(profile.method_verifications.is_empty());

        // Re-serialized, the empty field is omitted, so canonical bytes — and any
        // pre-existing signature over them — are unchanged.
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("method_verifications"));
    }
}
