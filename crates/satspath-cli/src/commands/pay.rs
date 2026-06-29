use anyhow::Result;

use satspath_core::{
    crypto::verify_signed_profile,
    PaymentMethod,
};
use satspath_router::{select_route, RouteRequest};

use super::open_registry;

pub async fn cmd_pay(alias: &str, amount_sats: u64) -> Result<()> {
    println!("─────────────────────────────────────────");
    println!("SatsPath Payment Simulation");
    println!("─────────────────────────────────────────");

    println!("Resolving alias '{}'...", alias);
    let registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("  Found profile.");

    println!("Verifying signed profile...");
    if !verify_signed_profile(signed)? {
        anyhow::bail!("Profile signature FAILED. Aborting payment.");
    }
    println!("  Signature valid.");

    println!("Checking available payment rails...");
    let req = RouteRequest {
        alias: alias.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let quote = select_route(&req)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("  Route selected: {}", quote.selected_method.method_name());

    println!();
    println!("Selected route: {}", quote.selected_method.method_name());
    println!("Reason:         {}", quote.reason);
    if let Some(fee) = quote.estimated_fee_sats {
        println!("Estimated fee:  {} sats", fee);
    }

    println!();
    println!("Executing simulated payment of {} sats to {}...", amount_sats, alias);

    // Simulate payment based on selected rail
    match &quote.selected_method {
        PaymentMethod::Lightning { lightning_address, .. } => {
            println!(
                "  [Lightning] Generating invoice from {}...",
                lightning_address.as_deref().unwrap_or("LNURL")
            );
            println!("  [Lightning] Invoice received.");
            println!("  [Lightning] Sending payment...");
        }
        PaymentMethod::Onchain { address, .. } => {
            println!("  [On-chain] Building transaction to {}...", address);
            println!("  [On-chain] Transaction signed (simulated).");
            println!("  [On-chain] Broadcast (simulated).");
        }
        PaymentMethod::Ark { server, pubkey, .. } => {
            println!(
                "  [Ark] Connecting to Ark server {}...",
                server
            );
            println!("  [Ark] Creating virtual UTXO for pubkey {}...", &pubkey[..16]);
            println!("  [Ark] Payment registered in Ark round (simulated).");
        }
    }

    println!();
    println!("Payment status: simulated_success");
    println!();
    println!("DISCLAIMER: This is a simulation. No real Bitcoin was moved.");
    Ok(())
}
