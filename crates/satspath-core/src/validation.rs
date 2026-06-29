use bitcoin::{Address, Network};
use secp256k1::PublicKey;
use std::str::FromStr;

use crate::errors::{Result, SatsPathError};
use crate::pointer::BitcoinNetwork;

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
}
