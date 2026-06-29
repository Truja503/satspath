use anyhow::Result;

use satspath_core::{
    crypto::{fingerprint_pubkey, generate_identity_keypair, sign_profile},
    privacy::{canonical_identifier, mask_identifier, mask_pubkey},
    validation::{
        validate_bitcoin_address, validate_compressed_pubkey, validate_lightning_address,
    },
    BitcoinNetwork, PaymentMethod, PaymentProfile,
};

use super::open_registry;

pub fn cmd_register(
    alias: &str,
    lightning_address: Option<&str>,
    onchain_address: Option<&str>,
    ark_server: Option<&str>,
    ark_pubkey: Option<&str>,
) -> Result<()> {
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


    let lightning_address = lightning_address.unwrap_or(&alias);
    validate_lightning_address(lightning_address).map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut methods = vec![PaymentMethod::Lightning {
        label: "Lightning Address".into(),
        lightning_address: Some(lightning_address.to_string()),
        lnurl: None,
        bolt12: None,
        receiver_pubkey: None,
    }];

    if let Some(address) = onchain_address {
        validate_bitcoin_address(address, BitcoinNetwork::Mainnet)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        methods.push(PaymentMethod::Onchain {
            label: "Bitcoin mainnet".into(),
            network: BitcoinNetwork::Mainnet,
            address: address.to_string(),
            pubkey_hint: None,
            descriptor_hint: None,
        });
    }

    match (ark_server, ark_pubkey) {
        (Some(server), Some(pubkey)) => {
            validate_compressed_pubkey(pubkey).map_err(|e| anyhow::anyhow!("{}", e))?;
            methods.push(PaymentMethod::Ark {
                label: "Ark".into(),
                server: server.to_string(),
                pubkey: pubkey.to_string(),
                vtxo_pointer: None,
            });
        }
        (None, None) => {}
        _ => anyhow::bail!("--ark-server and --ark-pubkey must be provided together."),
    }

    let profile = PaymentProfile {
        alias: alias.clone(),
        identity_pubkey: pubkey_hex.clone(),
        methods: methods.clone(),
        updated_at: chrono::Utc::now().timestamp(),
        expires_at: None,
        method_verifications: Vec::new(),
    };

    let signed = sign_profile(profile, &kp.secret_key)?;
    let fp = fingerprint_pubkey(&pubkey_hex)?;
    registry.register_profile(signed)?;

    println!("Registered: {}", mask_identifier(&alias));
    println!("Identity pubkey: {}", mask_pubkey(&pubkey_hex));
    println!("Fingerprint:     {}", fp);
    println!();
    println!("Payment methods registered:");
    for method in methods {
        println!("  - {}", method.method_name());
    }
    println!();
    println!("Profile signed and stored in .satspath/registry.json");
    println!("No private key stored.");
    Ok(())
}
