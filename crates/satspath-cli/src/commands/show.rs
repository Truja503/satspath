use anyhow::Result;

use satspath_core::{
    crypto::{fingerprint_pubkey, verify_signed_profile},
    privacy::{mask_address, mask_identifier, mask_invoice, mask_pubkey},
    PaymentMethod,
};

use super::get_resolver;
use satspath_core::resolver::ProfileResolver;

pub async fn cmd_show(alias: &str) -> Result<()> {
    let resolver = get_resolver()?;
    let signed = resolver
        .resolve_alias(alias)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let valid = verify_signed_profile(&signed)?;
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
                receiver_pubkey,
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
                if let Some(pubkey) = receiver_pubkey {
                    println!("      Receiver pubkey: {}", mask_pubkey(pubkey));
                }
            }
            PaymentMethod::Onchain {
                label,
                network,
                address,
                pubkey_hint,
                descriptor_hint,
            } => {
                println!("  - {} [On-chain]", label);
                println!("      Network: {:?}", network);
                println!("      Address: {}", mask_address(address));
                if let Some(hint) = pubkey_hint {
                    println!("      Pubkey hint: {}", mask_pubkey(hint));
                }
                if descriptor_hint.is_some() {
                    println!("      Descriptor hint: present");
                }
            }
            PaymentMethod::Ark {
                label,
                server,
                pubkey,
                vtxo_pointer,
            } => {
                println!("  - {} [Ark]", label);
                println!("      Server: {}", mask_address(server));
                println!("      Pubkey: {}", mask_pubkey(pubkey));
                if vtxo_pointer.is_some() {
                    println!("      VTXO pointer: present");
                }
            }
        }
    }
    Ok(())
}
