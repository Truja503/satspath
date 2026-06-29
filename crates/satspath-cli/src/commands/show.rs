use anyhow::Result;

use satspath_core::{
    crypto::{fingerprint_pubkey, verify_signed_profile},
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

    println!("Alias:          {}", signed.profile.alias);
    println!("Identity pubkey:{}", signed.profile.identity_pubkey);
    println!("Fingerprint:    {}", fp);
    println!(
        "Signature valid: {}",
        if valid { "yes" } else { "NO — profile may be tampered!" }
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
                    println!("      Lightning Address: {}", la);
                }
                if let Some(url) = lnurl {
                    println!("      LNURL: {}", url);
                }
                if let Some(b12) = bolt12 {
                    println!("      BOLT12: {}", b12);
                }
            }
            PaymentMethod::Onchain {
                label,
                address,
                pubkey_hint,
            } => {
                println!("  - {} [On-chain]", label);
                println!("      Address: {}", address);
                if let Some(hint) = pubkey_hint {
                    println!("      Pubkey hint: {}", hint);
                }
            }
            PaymentMethod::Ark { label, server, pubkey } => {
                println!("  - {} [Ark]", label);
                println!("      Server: {}", server);
                println!("      Pubkey: {}", pubkey);
            }
        }
    }
    Ok(())
}
