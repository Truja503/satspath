use anyhow::Result;

use satspath_core::{
    crypto::{fingerprint_pubkey, verify_signed_profile},
    privacy::{mask_address, mask_identifier, mask_invoice, mask_pubkey},
    PaymentMethod,
};

use super::open_registry;

pub fn cmd_show(alias: &str) -> Result<()> {
    let registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let valid = verify_signed_profile(signed)?;
    let fp = fingerprint_pubkey(&signed.profile.identity_pubkey)?;

    println!("Alias:          {}", mask_identifier(&signed.profile.alias));
    println!(
        "Identity pubkey:{}",
        mask_pubkey(&signed.profile.identity_pubkey)
    );
    println!("Fingerprint:    {}", fp);
    println!(
        "Signature valid: {}",
        if valid {
            "yes"
        } else {
            "NO — profile may be tampered!"
        }
    );
    println!("Updated at:     {}", signed.profile.updated_at);
    println!();
    println!("Methods:");
    for method in &signed.profile.methods {
        match method {
            PaymentMethod::Lightning {
                label,
                lightning_address,
                lnurl,
                bolt12,
            } => {
                println!("  - {} [Lightning]", label);
                if let Some(la) = lightning_address {
                    println!("      Lightning Address: {}", mask_identifier(la));
                }
                if let Some(url) = lnurl {
                    println!("      LNURL: {}", mask_address(url));
                }
                if let Some(b12) = bolt12 {
                    println!("      BOLT12: {}", mask_invoice(b12));
                }
            }
            PaymentMethod::Onchain {
                label,
                address,
                pubkey_hint,
            } => {
                println!("  - {} [On-chain]", label);
                println!("      Address: {}", mask_address(address));
                if let Some(hint) = pubkey_hint {
                    println!("      Pubkey hint: {}", mask_pubkey(hint));
                }
            }
            PaymentMethod::Ark {
                label,
                server,
                pubkey,
            } => {
                println!("  - {} [Ark]", label);
                println!("      Server: {}", mask_address(server));
                println!("      Pubkey: {}", mask_pubkey(pubkey));
            }
        }
    }
    Ok(())
}
