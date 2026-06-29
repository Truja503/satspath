use anyhow::Result;

use satspath_core::crypto::verify_signed_profile;
use satspath_router::{select_route, RouteRequest};

use super::get_resolver;
use satspath_core::resolver::ProfileResolver;

pub async fn cmd_quote(alias: &str, amount_sats: u64) -> Result<()> {
    let resolver = get_resolver()?;

    println!("Resolving alias '{}'...", alias);
    let signed = resolver
        .resolve_alias(alias)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Verifying signature...");
    if !verify_signed_profile(&signed)? {
        anyhow::bail!("Signature verification FAILED. Profile may be tampered.");
    }
    println!("Signature valid.");

    println!("Checking payment rails for {} sats...", amount_sats);
    let req = RouteRequest {
        alias: alias.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let quote = select_route(&req)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!();
    println!("Route Quote:");
    println!("  Selected rail:   {}", quote.selected_method.method_name());
    println!("  Label:           {}", quote.selected_method.label());
    println!("  Reason:          {}", quote.reason);
    if let Some(fee) = quote.estimated_fee_sats {
        println!("  Estimated fee:   {} sats", fee);
    }
    if let Some(conf) = &quote.estimated_confirmation {
        println!("  Confirmation:    {}", conf);
    }
    Ok(())
}
