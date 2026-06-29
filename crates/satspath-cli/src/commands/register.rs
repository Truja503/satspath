use anyhow::Result;

use satspath_core::{
    crypto::{fingerprint_pubkey, generate_identity_keypair, sign_profile},
    privacy::{canonical_identifier, mask_identifier, mask_pubkey},
    PaymentMethod, PaymentProfile,
};

use super::open_registry;

pub fn cmd_register(alias: &str) -> Result<()> {
    let mut registry = open_registry()?;
    let alias = canonical_identifier(alias);

    if registry.is_registered(&alias) {
        println!("Alias '{}' is already registered.", mask_identifier(&alias));
        return Ok(());
    }

    // Generate a fresh identity keypair for signing this public profile only.
    // The private key is not stored, printed, or encoded into any QR payload.
    let kp = generate_identity_keypair();
    let pubkey_hex = hex::encode(kp.public_key.serialize());

    // Build a demo profile with placeholder payment methods.
    // Multiple on-chain addresses are supported for privacy.
    let domain = alias.splitn(2, '@').nth(1).unwrap_or("example.com");
    let profile = PaymentProfile {
        alias: alias.clone(),
        identity_pubkey: pubkey_hex.clone(),
        methods: vec![
            PaymentMethod::Lightning {
                label: "Lightning Address".into(),
                lnurl: None,
                lightning_address: Some(alias.clone()),
                bolt12: None,
            },
            // First on-chain address (primary)
            PaymentMethod::Onchain {
                label: "Bitcoin (primary)".into(),
                address: format!("bc1q{}placeholder0001", &pubkey_hex[..8]),
                pubkey_hint: Some(pubkey_hex[..16].to_string()),
            },
            // Second on-chain address for privacy
            PaymentMethod::Onchain {
                label: "Bitcoin (secondary)".into(),
                address: format!("bc1q{}placeholder0002", &pubkey_hex[8..16]),
                pubkey_hint: Some(pubkey_hex[16..32].to_string()),
            },
            PaymentMethod::Ark {
                label: "Ark".into(),
                server: format!("ark.{}", domain),
                pubkey: pubkey_hex.clone(),
            },
        ],
        updated_at: chrono::Utc::now().timestamp(),
    };

    let signed = sign_profile(profile, &kp.secret_key)?;
    let fp = fingerprint_pubkey(&pubkey_hex)?;

    registry.register_profile(signed)?;

    println!("Registered: {}", mask_identifier(&alias));
    println!("Identity pubkey: {}", mask_pubkey(&pubkey_hex));
    println!("Fingerprint:     {}", fp);
    println!();
    println!("Payment methods registered:");
    println!("  - Lightning Address");
    println!("  - Bitcoin on-chain (primary)");
    println!("  - Bitcoin on-chain (secondary, privacy address)");
    println!("  - Ark");
    println!();
    println!("Profile signed and stored in .satspath/registry.json");
    println!("No private key stored.");
    Ok(())
}
