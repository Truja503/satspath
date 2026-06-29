use anyhow::Result;
use serde_json::{json, Value};
use std::fs;

use satspath_core::{
    crypto::{fingerprint_pubkey, generate_identity_keypair, sign_profile},
    PaymentMethod, PaymentProfile,
};

use super::{open_registry, satspath_dir};

pub fn cmd_register(alias: &str) -> Result<()> {
    let mut registry = open_registry()?;

    if registry.is_registered(alias) {
        println!("Alias '{}' is already registered.", alias);
        return Ok(());
    }

    // Generate a fresh identity keypair (demo only — keys stored locally).
    let kp = generate_identity_keypair();
    let pubkey_hex = hex::encode(kp.public_key.serialize());
    let secret_hex = hex::encode(kp.secret_key.secret_bytes());

    // Build a demo profile with placeholder payment methods.
    // Multiple on-chain addresses are supported for privacy.
    let domain = alias.splitn(2, '@').nth(1).unwrap_or("example.com");
    let profile = PaymentProfile {
        alias: alias.to_string(),
        identity_pubkey: pubkey_hex.clone(),
        methods: vec![
            PaymentMethod::Lightning {
                label: "Lightning Address".into(),
                lnurl: None,
                lightning_address: Some(alias.to_string()),
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

    // Persist the demo secret key locally (never commit this file).
    let keys_path = satspath_dir().join("keys.json");
    let mut keys: Value = if keys_path.exists() {
        serde_json::from_str(&fs::read_to_string(&keys_path)?)?
    } else {
        json!({ "warning": "DEMO keys only.", "keys": {} })
    };
    keys["keys"][alias] = json!({
        "pubkey": pubkey_hex,
        "secret_key_hex": secret_hex,
        "warning": "DEMO USE ONLY. Never use with real funds.",
    });
    fs::write(&keys_path, serde_json::to_string_pretty(&keys)?)?;

    println!("Registered: {}", alias);
    println!("Identity pubkey: {}", pubkey_hex);
    println!("Fingerprint:     {}", fp);
    println!();
    println!("Payment methods registered:");
    println!("  - Lightning Address");
    println!("  - Bitcoin on-chain (primary)");
    println!("  - Bitcoin on-chain (secondary, privacy address)");
    println!("  - Ark");
    println!();
    println!("Profile signed and stored in .satspath/registry.json");
    println!("DEMO secret key stored in .satspath/keys.json (git-ignored)");
    Ok(())
}
