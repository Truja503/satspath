use anyhow::Result;

use satspath_core::{
    crypto::verify_signed_profile,
    evaluate_method_trust_for_profile,
    privacy::{mask_address, mask_identifier, mask_invoice, mask_pubkey},
    resolver::ProfileResolver,
    PaymentMethod,
};
use satspath_router::{
    fetch_invoice, fetch_lnurl_metadata, lightning::lightning_address, select_route, RouteRequest,
};

use super::{
    get_resolver,
    qr::{bitcoin_uri, print_qr},
};

pub async fn cmd_quote(alias: &str, amount_sats: u64) -> Result<()> {
    let resolver = get_resolver()?;

    println!("Resolving identifier '{}'...", mask_identifier(alias));
    let signed = resolver
        .resolve_alias(alias)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("found.");

    print!("Verifying signature... ");
    if !verify_signed_profile(&signed)? {
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

    if let Some(snap) = &quote.fee_snapshot {
        println!();
        println!("  Mempool fees (sat/vB)");
        println!("  ├─ Next block  (~10 min): {} sat/vB", snap.fastest_sat_vb);
        println!(
            "  ├─ 30 minutes           : {} sat/vB",
            snap.half_hour_sat_vb
        );
        println!("  └─ 60 minutes           : {} sat/vB", snap.hour_sat_vb);
    }

    println!();
    println!("  ┌─────────────────────────────────────────┐");
    println!(
        "  │  Rail   : {:30}  │",
        quote.selected_method.method_name()
    );
    println!("  │  Label  : {:30}  │", quote.selected_method.label());
    if let Some(fee) = quote.estimated_fee_sats {
        println!("  │  Fee    : {:30}  │", format!("{} sats", fee));
    }
    if let Some(conf) = &quote.estimated_confirmation {
        println!("  │  Confirm: {:30}  │", conf);
    }
    println!("  └─────────────────────────────────────────┘");
    println!("  Reason: {}", quote.reason);

    // Ownership trust of the selected rail, re-verified client-side. Unifies
    // method_verifications and any inline Ark pointer proof under one signal.
    let trust = evaluate_method_trust_for_profile(
        &signed.profile,
        &quote.selected_method,
        chrono::Utc::now().timestamp(),
        None,
    );
    println!("  Ownership: {}", trust.badge());
    if trust.is_suspicious() {
        println!("  ⚠  This rail's ownership proof did not verify — treat with caution.");
    }

    println!();
    match &quote.selected_method {
        PaymentMethod::Lightning { .. } => {
            let ln_addr = lightning_address(&quote.selected_method)
                .ok_or_else(|| anyhow::anyhow!("no Lightning Address in method"))?;

            print!("  Fetching invoice from {}... ", mask_identifier(ln_addr));
            match fetch_lnurl_metadata(ln_addr).await {
                Ok(meta) => {
                    match fetch_invoice(&meta, amount_sats, None).await {
                        Ok(invoice) => {
                            println!("received.");
                            println!();
                            println!("  Scan to pay — Lightning ({} sats)", amount_sats);
                            println!("  ─────────────────────────────────────────");
                            print_qr(&invoice.to_uppercase())?;
                            println!("  {}...", mask_invoice(&invoice));
                            println!("  Warning: this is a real invoice QR generated from public metadata.");
                        }
                        Err(e) => {
                            println!("unavailable ({e}).");
                            println!("  Preview only. No funds moved.");
                        }
                    }
                }
                Err(e) => {
                    println!("unavailable ({e}).");
                    println!("  Preview only. No funds moved.");
                }
            }
        }
        PaymentMethod::Onchain { address, .. } => {
            let uri = bitcoin_uri(address, amount_sats);
            println!("  Scan to pay — Bitcoin on-chain ({} sats)", amount_sats);
            println!("  ─────────────────────────────────────────");
            print_qr(&uri)?;
            println!("  {}", mask_address(&uri));
        }
        PaymentMethod::Ark { pubkey, server, .. } => {
            println!("  Ark payment via {}", mask_address(server));
            println!("  Pubkey: {}", mask_pubkey(pubkey));
            println!("  Use `satspath pay --mainnet-preview` for public pointer preview.");
        }
    }

    Ok(())
}
