use anyhow::Result;

use satspath_core::{crypto::verify_signed_profile, PaymentMethod};
use satspath_router::{
    fetch_invoice, fetch_lnurl_metadata, lightning::lightning_address, select_route, RouteRequest,
};

use super::{open_registry, qr::{bitcoin_uri, print_qr}};

pub async fn cmd_quote(alias: &str, amount_sats: u64) -> Result<()> {
    let registry = open_registry()?;

    print!("Resolving '{}'... ", alias);
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("found.");

    print!("Verifying signature... ");
    if !verify_signed_profile(signed)? {
        anyhow::bail!("Signature INVALID. Profile may be tampered.");
    }
    println!("valid.");

    print!("Fetching mempool fees + selecting rail... ");
    let req = RouteRequest {
        alias: alias.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let quote = select_route(&req)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("done.");

    // ── Fee table (when we have a snapshot) ────────────────────────────────
    if let Some(snap) = &quote.fee_snapshot {
        println!();
        println!("  Mempool fees (sat/vB)");
        println!("  ├─ Next block  (~10 min): {} sat/vB", snap.fastest_sat_vb);
        println!("  ├─ 30 minutes           : {} sat/vB", snap.half_hour_sat_vb);
        println!("  └─ 60 minutes           : {} sat/vB", snap.hour_sat_vb);
    }

    // ── Routing decision ────────────────────────────────────────────────────
    println!();
    println!("  ┌─────────────────────────────────────────┐");
    println!("  │  Rail   : {:30}  │", quote.selected_method.method_name());
    println!("  │  Label  : {:30}  │", quote.selected_method.label());
    if let Some(fee) = quote.estimated_fee_sats {
        println!("  │  Fee    : {:30}  │", format!("{} sats", fee));
    }
    if let Some(conf) = &quote.estimated_confirmation {
        println!("  │  Confirm: {:30}  │", conf);
    }
    println!("  └─────────────────────────────────────────┘");
    println!("  Reason: {}", quote.reason);

    // ── QR for the selected method ──────────────────────────────────────────
    println!();
    match &quote.selected_method {
        PaymentMethod::Lightning { .. } => {
            let ln_addr = lightning_address(&quote.selected_method)
                .ok_or_else(|| anyhow::anyhow!("no Lightning Address in method"))?;

            print!("  Fetching real invoice from {}... ", ln_addr);
            let meta = fetch_lnurl_metadata(ln_addr).await?;
            let invoice = fetch_invoice(&meta, amount_sats, None).await?;
            println!("received.");

            println!();
            println!("  Scan to pay — Lightning invoice ({} sats)", amount_sats);
            println!("  ─────────────────────────────────────────");
            // BOLT11 uppercase = alphanumeric QR mode → smaller, easier to scan
            print_qr(&invoice.to_uppercase())?;
            println!("  {}", &invoice[..40]);
            println!("  ...{}", &invoice[invoice.len().saturating_sub(20)..]);
        }

        PaymentMethod::Onchain { address, .. } => {
            let uri = bitcoin_uri(address, amount_sats);
            println!("  Scan to pay — Bitcoin on-chain ({} sats)", amount_sats);
            println!("  ─────────────────────────────────────────");
            print_qr(&uri)?;
            println!("  {}", uri);
        }

        PaymentMethod::Ark { pubkey, server, .. } => {
            println!("  Ark payment via {}", server);
            println!("  Pubkey: {}", pubkey);
            println!("  (Ark QR coming once real Ark client is integrated)");
        }
    }

    Ok(())
}
