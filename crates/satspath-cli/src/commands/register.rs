use anyhow::Result;
use serde_json::{json, Value};
use std::fs;

use satspath_core::{
    crypto::{fingerprint_pubkey, generate_identity_keypair, sign_profile},
    display_hint,
    PaymentMethod, PaymentProfile,
};

use super::{open_registry, satspath_dir};

pub fn cmd_register(
    alias: &str,
    ln_address: Option<&str>,
    onchain_address: Option<&str>,
) -> Result<()> {
    let mut registry = open_registry()?;

    if registry.is_registered(alias) {
        println!("Alias '{}' is already registered.", alias);
        return Ok(());
    }

    let kp = generate_identity_keypair();
    let pubkey_hex = hex::encode(kp.public_key.serialize());
    let secret_hex = hex::encode(kp.secret_key.secret_bytes());
    let domain = alias.splitn(2, '@').nth(1).unwrap_or("example.com");

    let effective_ln = ln_address.unwrap_or(alias);
    let mut methods: Vec<PaymentMethod> = vec![PaymentMethod::Lightning {
        label: if ln_address.is_some() {
            format!("Lightning Address ({})", effective_ln)
        } else {
            "Lightning Address".into()
        },
        lnurl: None,
        lightning_address: Some(effective_ln.to_string()),
        bolt12: None,
    }];

    if let Some(addr) = onchain_address {
        methods.push(PaymentMethod::Onchain {
            label: "Bitcoin (primary)".into(),
            address: addr.to_string(),
            pubkey_hint: Some(pubkey_hex[..16].to_string()),
        });
    } else {
        methods.push(PaymentMethod::Onchain {
            label: "Bitcoin (primary — placeholder)".into(),
            address: format!("bc1q{}placeholder0001", &pubkey_hex[..8]),
            pubkey_hint: Some(pubkey_hex[..16].to_string()),
        });
        methods.push(PaymentMethod::Onchain {
            label: "Bitcoin (secondary — placeholder)".into(),
            address: format!("bc1q{}placeholder0002", &pubkey_hex[8..16]),
            pubkey_hint: Some(pubkey_hex[16..32].to_string()),
        });
    }

    methods.push(PaymentMethod::Ark {
        label: "Ark".into(),
        server: format!("ark.{}", domain),
        pubkey: pubkey_hex.clone(),
    });

    let profile = PaymentProfile {
        alias: alias.to_string(),
        identity_pubkey: pubkey_hex.clone(),
        methods,
        updated_at: chrono::Utc::now().timestamp(),
    };

    let signed = sign_profile(profile, &kp.secret_key)?;
    let fp = fingerprint_pubkey(&pubkey_hex)?;
    registry.register_profile(signed)?;

    let keys_path = satspath_dir().join("keys.json");
    let mut keys: Value = if keys_path.exists() {
        serde_json::from_str(&fs::read_to_string(&keys_path)?)?
    } else {
        json!({ "warning": "DEMO keys only. Never use with real funds.", "keys": {} })
    };
    keys["keys"][alias] = json!({
        "pubkey": pubkey_hex,
        "secret_key_hex": secret_hex,
        "warning": "DEMO USE ONLY.",
    });
    fs::write(&keys_path, serde_json::to_string_pretty(&keys)?)?;

    println!("Registered: {} ({})", alias, display_hint(alias));
    println!("Identity pubkey: {}...", &pubkey_hex[..16]);
    println!("Fingerprint:     {}", fp);
    if let Some(la) = ln_address {
        println!("Lightning wired: {} → {}", alias, la);
    }
    println!();
    println!("Profile signed and stored in .satspath/registry.json");
    Ok(())
}
