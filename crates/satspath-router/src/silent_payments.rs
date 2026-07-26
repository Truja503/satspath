use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey, Scalar};
use bitcoin::Network;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use bech32::ToBase32;
use sha2::{Sha256, Digest};
use rand::rngs::OsRng;

/// Silent Payment (BIP-352) Scan Public Key (sp1q...)
/// This is the scan key that allows the recipient to detect payments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SilentPaymentScanKey {
    /// The scan public key (33 bytes compressed, bech32m encoded as sp1q...)
    pub scan_pubkey: String,
    /// The spend public key (33 bytes compressed, bech32m encoded as sp1q...)
    /// Optional - if None, derived from scan key
    pub spend_pubkey: Option<String>,
}

/// Silent Payment Address derived from scan and spend keys
/// BIP-352: silent payment address = sp1q + bech32m(data)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SilentPaymentAddress {
    /// The silent payment address (starts with sp1q...)
    pub address: String,
    /// The scan public key used to derive this address
    pub scan_pubkey: String,
    /// The spend public key used to derive this address
    pub spend_pubkey: String,
    /// Label for the address
    pub label: Option<String>,
}

/// Silent Payment Output - an output that can be detected by the recipient
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SilentPaymentOutput {
    /// The output script (P2TR)
    pub script_pubkey: String,
    /// The amount in satoshis
    pub amount_sats: u64,
    /// The tweaked public key used for this output
    pub tweaked_pubkey: String,
    /// The shared secret used to derive the tweak
    pub shared_secret: Option<String>, // For debugging/verification
}

/// Silent Payment Input - used when creating a silent payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilentPaymentInput {
    /// The outpoint being spent
    pub outpoint: String, // txid:vout
    /// The taproot public key of the input (for key tweaking)
    pub input_pubkey: String,
    /// The private key corresponding to the input (for signer)
    #[serde(skip_serializing)]
    pub input_privkey: Option<String>,
}

/// Silent Payment - a payment using BIP-352 Silent Payments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilentPayment {
    /// The scan public key of the recipient
    pub scan_pubkey: String,
    /// The spend public key of the recipient
    pub spend_pubkey: String,
    /// The amount in satoshis
    pub amount_sats: u64,
    /// The inputs being spent
    pub inputs: Vec<SilentPaymentInput>,
    /// The change output (optional)
    pub change_output: Option<SilentPaymentOutput>,
    /// The network
    pub network: Network,
}

/// BIP-352 Silent Payment derivation
/// 
/// Silent Payment address generation:
/// 1. Recipient has scan key (scan_pubkey) and spend key (spend_pubkey)
/// 2. Sender creates a shared secret: sender_privkey * recipient_scan_pubkey
/// 3. Shared secret is used to tweak the recipient's spend_pubkey
/// 3. Sender creates a P2TR output with the tweaked public key
/// 4. Recipient scans blockchain using their scan key to find outputs
impl SilentPayment {
    /// Create a new silent payment
    pub fn new(
        scan_pubkey: String,
        spend_pubkey: String,
        amount_sats: u64,
        inputs: Vec<SilentPaymentInput>,
        change_output: Option<SilentPaymentOutput>,
        network: Network,
    ) -> Self {
        Self {
            scan_pubkey,
            spend_pubkey,
            amount_sats,
            inputs,
            change_output,
            network,
        }
    }
    
    /// Create the silent payment address for the recipient
/// This is the address the sender would use to send to the recipient
    pub fn recipient_address(scan_pubkey: &str, spend_pubkey: &str) -> Result<String, anyhow::Error> {
        // BIP-352: Silent Payment address = sp1q + bech32m(spend_pubkey + scan_pubkey)
        // This is a simplified version - full implementation needs bech32m encoding
        let combined = format!("{}{}", spend_pubkey, scan_pubkey);
        // In practice, this uses bech32m encoding with "sp" prefix
        Ok(format!("sp1q{}", combined[..70].to_lowercase())) // Placeholder
    }
    
    /// Derive the tweaked public key for a silent payment output
/// 
/// shared_secret = sender_privkey * recipient_scan_pubkey
/// tweaked_pubkey = recipient_spend_pubkey + hash(shared_secret) * G
pub fn derive_tweaked_pubkey(
    sender_privkey: &SecretKey,
    recipient_scan_pubkey: &PublicKey,
    recipient_spend_pubkey: &PublicKey,
) -> Result<PublicKey, anyhow::Error> {
    let secp = Secp256k1::new();
    
    // Shared secret = sender_privkey * recipient_scan_pubkey
    let sender_scalar = Scalar::from_be_bytes(sender_privkey.secret_bytes()).map_err(|e| anyhow!("Invalid scalar: {}", e))?;
    let shared_secret = recipient_scan_pubkey.mul_tweak(&secp, &sender_scalar)
        .map_err(|e| anyhow!("Failed to compute shared secret: {}", e))?;
    
    // Hash the shared secret's x-coordinate to get a scalar for tweaking
    let shared_secret_bytes = shared_secret.serialize();
    let mut hasher = Sha256::new();
    hasher.update(&shared_secret_bytes[1..]); // Skip the 0x02/0x03 prefix, use x-coordinate only
    let tweak_bytes: [u8; 32] = hasher.finalize().into();
    let tweak_scalar = Scalar::from_be_bytes(tweak_bytes).map_err(|e| anyhow!("Invalid tweak scalar: {}", e))?;
    
    // Tweaked pubkey = recipient_spend_pubkey + tweak_scalar * G
    let tweaked = recipient_spend_pubkey.add_exp_tweak(&secp, &tweak_scalar)
        .map_err(|e| anyhow!("Failed to tweak public key: {}", e))?;
    
    Ok(tweaked)
}
    
    /// Create a silent payment output
    /// 
    /// The sender:
    /// 1. Computes shared secret = sender_privkey * recipient_scan_pubkey
    /// 2. Tweaks recipient's spend_pubkey with shared_secret
    /// 3. Creates P2TR output with tweaked pubkey
    pub fn create_output(
        &self,
        _recipient_scan_pubkey: &PublicKey,
        _recipient_spend_pubkey: &PublicKey,
    ) -> Result<SilentPaymentOutput, anyhow::Error> {
        // This requires the sender's private key which should be in the inputs
        // For now, return a placeholder
        Ok(SilentPaymentOutput {
            script_pubkey: "tr(...)".to_string(),
            amount_sats: self.amount_sats,
            tweaked_pubkey: "...".to_string(),
            shared_secret: None,
        })
    }
    
    /// Detect silent payments addressed to the recipient
    /// 
    /// The recipient scans the blockchain:
    /// 1. For each transaction, compute shared_secret = scan_privkey * input_pubkey
    /// 2. For each output, check if output_pubkey == spend_pubkey + shared_secret * G
    /// 3. If match, the output belongs to the recipient
    pub fn detect_outputs(
        _scan_privkey: &SecretKey,
        _spend_pubkey: &PublicKey,
        tx_outputs: &[(String, u64)], // (script_pubkey, amount_sats)
    ) -> Result<Vec<SilentPaymentOutput>, anyhow::Error> {
        let _secp = Secp256k1::new();
        let detected = Vec::new();
        
        for (script_pubkey, _amount_sats) in tx_outputs {
            // Check if this is a P2TR output
            if script_pubkey.starts_with("5120") { // P2TR marker
                // Extract the public key from the script
                let output_pubkey_hex = &script_pubkey[4..70]; // Skip "5120" (OP_1 OP_32)
                let _output_pubkey = PublicKey::from_slice(&hex::decode(output_pubkey_hex)?)
                    .map_err(|e| anyhow!("Failed to parse output pubkey: {}", e))?;
                
                // We need the input pubkeys to compute shared secrets
                // This is simplified - in practice, you'd iterate over all inputs
                // For now, this is a placeholder
            }
        }
        
        Ok(detected)
    }
}

/// Generate a new silent payment key pair (scan + spend)
pub fn generate_silent_payment_keys() -> Result<(String, String, String, String), anyhow::Error> {
    let secp = Secp256k1::new();
    
    // Generate scan key pair
    let scan_privkey = SecretKey::new(&mut OsRng);
    let scan_pubkey = PublicKey::from_secret_key(&secp, &scan_privkey);
    
    // Generate spend key pair
    let spend_privkey = SecretKey::new(&mut OsRng);
    let spend_pubkey = PublicKey::from_secret_key(&secp, &spend_privkey);
    
    Ok((
        hex::encode(scan_privkey.secret_bytes()),  // scan private key
        hex::encode(scan_pubkey.serialize()),      // scan public key
        hex::encode(spend_privkey.secret_bytes()), // spend private key
        hex::encode(spend_pubkey.serialize()),     // spend public key
    ))
}

/// Parse a silent payment scan public key (sp1q...)
pub fn parse_silent_payment_scan_key(scan_key: &str) -> Result<PublicKey, anyhow::Error> {
    if !scan_key.starts_with("sp1q") {
        return Err(anyhow!("Invalid silent payment scan key: must start with 'sp1q'"));
    }
    
    // Decode bech32m (simplified)
    // In practice, use bech32 crate
    let data = &scan_key[4..]; // Remove "sp1q" prefix
    let bytes = hex::decode(data)?;
    PublicKey::from_slice(&bytes).map_err(|e| anyhow!("Invalid public key: {}", e))
}

/// Create a silent payment address from scan and spend public keys
pub fn create_silent_payment_address(scan_pubkey: &PublicKey, spend_pubkey: &PublicKey) -> Result<String, anyhow::Error> {
    // BIP-352: sp1q + bech32m(spend_pubkey || scan_pubkey)
    // The silent payment address format:
    // sp1q + bech32m(spend_pubkey_bytes || scan_pubkey_bytes)
    
    let mut combined = Vec::new();
    combined.extend_from_slice(&spend_pubkey.serialize());
    combined.extend_from_slice(&scan_pubkey.serialize());
    
    // Encode as bech32m with "sp" prefix
    // This is a simplified version - full implementation needs bech32 crate
    let encoded = bech32::encode("sp", combined.to_base32(), bech32::Variant::Bech32m)
        .map_err(|e| anyhow!("Failed to encode silent payment address: {}", e))?;
    
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    
    #[test]
    fn test_generate_silent_payment_keys() {
        let (scan_priv, scan_pub, spend_priv, spend_pub) = generate_silent_payment_keys().unwrap();
        assert_eq!(scan_priv.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(scan_pub.len(), 66);  // 33 bytes = 66 hex chars (compressed)
        assert_eq!(spend_priv.len(), 64);
        assert_eq!(spend_pub.len(), 66);
    }
    
    #[test]
    fn test_parse_silent_payment_scan_key() {
        let (_, scan_pub, _, _) = generate_silent_payment_keys().unwrap();
        // The parse function expects hex-encoded pubkey after "sp1q" prefix
        let sp1q = format!("sp1q{}", scan_pub.to_lowercase());
        let result = parse_silent_payment_scan_key(&sp1q);
        assert!(result.is_ok());
    }
}