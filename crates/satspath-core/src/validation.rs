use bitcoin::{Address, Network};
use secp256k1::PublicKey;
use std::str::FromStr;

use crate::ark::{validate_ark_receive_pointer, verify_ark_ownership_proof, ArkReceivePointer};
use crate::errors::{Result, SatsPathError};
use crate::pointer::BitcoinNetwork;
use crate::profile::{ClaimPolicy, PaymentMethod, PaymentProfile};

pub const LARGE_PREVIEW_AMOUNT_SATS: u64 = 100_000_000;
pub const MAX_PREVIEW_AMOUNT_SATS: u64 = 21_000_000 * 100_000_000;

pub fn validate_amount_sats(amount_sats: u64) -> Result<()> {
    if amount_sats == 0 {
        return Err(SatsPathError::InvalidPaymentPointer(
            "amount must be positive".into(),
        ));
    }
    if amount_sats > MAX_PREVIEW_AMOUNT_SATS {
        return Err(SatsPathError::InvalidPaymentPointer(
            "amount exceeds Bitcoin supply".into(),
        ));
    }
    Ok(())
}

pub fn validate_compressed_pubkey(pubkey_hex: &str) -> Result<()> {
    let bytes =
        hex::decode(pubkey_hex).map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    if bytes.len() != 33 || !matches!(bytes.first(), Some(0x02 | 0x03)) {
        return Err(SatsPathError::InvalidPublicKey(
            "expected 33-byte compressed secp256k1 pubkey".into(),
        ));
    }
    PublicKey::from_slice(&bytes).map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    Ok(())
}

pub fn validate_lightning_address(address: &str) -> Result<()> {
    let trimmed = address.trim();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return Err(SatsPathError::InvalidPaymentPointer(
            "Lightning Address must be user@domain".into(),
        ));
    };
    let valid_local = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'));
    let valid_domain = domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    if valid_local && valid_domain {
        Ok(())
    } else {
        Err(SatsPathError::InvalidPaymentPointer(
            "invalid Lightning Address format".into(),
        ))
    }
}

pub fn validate_bitcoin_address(address: &str, network: BitcoinNetwork) -> Result<()> {
    let parsed = Address::from_str(address.trim())
        .map_err(|e| SatsPathError::InvalidPaymentPointer(e.to_string()))?;
    let expected = match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    };
    parsed
        .require_network(expected)
        .map_err(|e| SatsPathError::InvalidPaymentPointer(e.to_string()))?;
    Ok(())
}

pub fn assert_no_private_material(payload: &str) -> Result<()> {
    let lower = payload.to_ascii_lowercase();
    let blocked = [
        "xprv",
        "tprv",
        "seed phrase",
        "mnemonic",
        "private_key",
        "private key",
        "secret_key",
        "macaroon",
        "cert",
        "api_key",
        "apikey",
        "secret",
        "password",
    ];
    if let Some(term) = blocked.iter().find(|term| lower.contains(**term)) {
        return Err(SatsPathError::PrivateMaterialRejected((*term).into()));
    }

    let word_count = payload
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| word.len() >= 3)
        .count();
    if word_count >= 12 && lower.contains("seed") {
        return Err(SatsPathError::PrivateMaterialRejected(
            "seed phrase-like payload".into(),
        ));
    }
    Ok(())
}

pub fn validate_claim_policy(policy: &ClaimPolicy) -> Result<()> {
    match policy {
        ClaimPolicy::SingleSig { receiver_pubkey } => validate_compressed_pubkey(receiver_pubkey),
        ClaimPolicy::Multisig {
            threshold,
            pubkeys,
            descriptor,
        } => {
            if *threshold == 0 || usize::from(*threshold) > pubkeys.len() {
                return Err(SatsPathError::InvalidPaymentPointer(
                    "invalid multisig threshold".into(),
                ));
            }
            for pubkey in pubkeys {
                validate_compressed_pubkey(pubkey)?;
            }
            if let Some(descriptor) = descriptor {
                assert_no_private_material(descriptor)?;
            }
            Ok(())
        }
        ClaimPolicy::FutureTaproot {
            internal_key,
            script_policy_hint,
        } => {
            validate_compressed_pubkey(internal_key)?;
            if let Some(hint) = script_policy_hint {
                assert_no_private_material(hint)?;
            }
            Ok(())
        }
    }
}

pub fn validate_public_profile(profile: &PaymentProfile) -> Result<()> {
    assert_no_private_material(&profile.alias)?;
    validate_compressed_pubkey(&profile.identity_pubkey)?;
    if let Some(expires_at) = profile.expires_at {
        if expires_at <= profile.updated_at {
            return Err(SatsPathError::InvalidPaymentPointer(
                "profile expires before it is updated".into(),
            ));
        }
    }

    for method in &profile.methods {
        match method {
            PaymentMethod::Lightning {
                lightning_address,
                lnurl,
                bolt12,
                receiver_pubkey,
                ..
            } => {
                if let Some(address) = lightning_address {
                    validate_lightning_address(address)?;
                }
                if let Some(url) = lnurl {
                    let parsed = url::Url::parse(url)
                        .map_err(|e| SatsPathError::InvalidPaymentPointer(e.to_string()))?;
                    if !matches!(parsed.scheme(), "https" | "http") {
                        return Err(SatsPathError::InvalidPaymentPointer(
                            "LNURL must be http(s)".into(),
                        ));
                    }
                }
                if let Some(invoice) = bolt12 {
                    assert_no_private_material(invoice)?;
                }
                if let Some(pubkey) = receiver_pubkey {
                    validate_compressed_pubkey(pubkey)?;
                }
            }
            PaymentMethod::Onchain {
                network,
                address,
                pubkey_hint,
                descriptor_hint,
                ..
            } => {
                validate_bitcoin_address(address, *network)?;
                if let Some(pubkey) = pubkey_hint {
                    validate_compressed_pubkey(pubkey)?;
                }
                if let Some(descriptor) = descriptor_hint {
                    assert_no_private_material(descriptor)?;
                }
            }
            PaymentMethod::Ark {
                server,
                pubkey,
                vtxo_pointer,
                proof,
                expires_at,
                ..
            } => {
                let pointer = ArkReceivePointer {
                    server: server.clone(),
                    receiver_pubkey: pubkey.clone(),
                    vtxo_pointer: vtxo_pointer.clone(),
                    proof: proof.clone(),
                    expires_at: *expires_at,
                };
                let now = chrono::Utc::now().timestamp();
                validate_ark_receive_pointer(&pointer, now)?;
                if proof.is_some() && !verify_ark_ownership_proof(&profile.alias, &pointer, now)? {
                    return Err(SatsPathError::InvalidSignature);
                }
            }
        }
    }

    // Ownership attestations are structurally validated here (no private
    // material, well-formed proofs, sane expiry). Cryptographic re-verification
    // is performed separately at resolve time by
    // `ownership::verify_method_verification`.
    for verification in &profile.method_verifications {
        crate::ownership::validate_method_verification(verification)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_PUBKEY: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    #[test]
    fn mainnet_address_accepted_in_mainnet_preview() {
        assert!(validate_bitcoin_address(
            "1BoatSLRHtKNngkdXEeobR76b53LETtpyT",
            BitcoinNetwork::Mainnet
        )
        .is_ok());
    }

    #[test]
    fn testnet_address_rejected_in_mainnet_preview() {
        assert!(validate_bitcoin_address(
            "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn",
            BitcoinNetwork::Mainnet
        )
        .is_err());
    }

    #[test]
    fn compressed_secp256k1_pubkey_accepted() {
        assert!(validate_compressed_pubkey(VALID_PUBKEY).is_ok());
    }

    #[test]
    fn invalid_pubkey_rejected() {
        assert!(validate_compressed_pubkey("04abcdef").is_err());
    }

    #[test]
    fn lightning_address_parsed() {
        assert!(validate_lightning_address("rodrigo@example.com").is_ok());
        assert!(validate_lightning_address("not-an-address").is_err());
    }

    #[test]
    fn profile_rejects_private_material() {
        let profile = PaymentProfile {
            alias: "alice@example.com".into(),
            identity_pubkey: VALID_PUBKEY.into(),
            methods: vec![PaymentMethod::Lightning {
                label: "LN".into(),
                lightning_address: Some("alice@example.com".into()),
                lnurl: None,
                bolt12: Some("secret xprv payload".into()),
                receiver_pubkey: None,
            }],
            updated_at: 1,
            expires_at: Some(2),
            method_verifications: Vec::new(),
        };
        assert!(validate_public_profile(&profile).is_err());
    }

    #[test]
    fn multisig_claim_policy_validates_pubkeys() {
        let policy = ClaimPolicy::Multisig {
            threshold: 2,
            pubkeys: vec![VALID_PUBKEY.into(), VALID_PUBKEY.into()],
            descriptor: Some("wsh(sortedmulti(2,...))".into()),
        };
        assert!(validate_claim_policy(&policy).is_ok());
    }
}
