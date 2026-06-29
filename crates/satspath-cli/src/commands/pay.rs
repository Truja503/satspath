use anyhow::Result;

use satspath_core::{crypto::verify_signed_profile, PaymentMethod};
use satspath_router::{
    fetch_invoice, fetch_lnurl_metadata, lightning::lightning_address, select_route, RouteRequest,
};

use super::{open_registry, qr::{bitcoin_uri, print_qr}};

pub async fn cmd_pay(alias: &str, amount_sats: u64, memo: Option<&str>) -> Result<()> {
    println!("══════════════════════════════════════════════════");
    println!("  SatsPath — Live Payment");
    println!("══════════════════════════════════════════════════");

    // ── Resolve ─────────────────────────────────────────────────────────────
    print!("Resolving {}... ", alias);
    let registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("found.");

    // ── Verify ──────────────────────────────────────────────────────────────
    print!("Verifying profile signature... ");
    if !verify_signed_profile(signed)? {
        anyhow::bail!("Signature INVALID — profile may be tampered. Aborting.");
    }
    println!("valid.");

    // ── Route ────────────────────────────────────────────────────────────────
    print!("Selecting rail for {} sats... ", amount_sats);
    let req = RouteRequest {
        alias: alias.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let quote = select_route(&req)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("done.");

    // Fee table
    if let Some(snap) = &quote.fee_snapshot {
        println!();
        println!("  Mempool fees (sat/vB)");
        println!("  ├─ Next block  (~10 min): {} sat/vB", snap.fastest_sat_vb);
        println!("  ├─ 30 minutes           : {} sat/vB", snap.half_hour_sat_vb);
        println!("  └─ 60 minutes           : {} sat/vB", snap.hour_sat_vb);
    }

    println!();
    println!("  Rail : {}", quote.selected_method.method_name());
    println!("  Why  : {}", quote.reason);
    if let Some(fee) = quote.estimated_fee_sats {
        println!("  Fee  : {} sats", fee);
    }
    if let Some(conf) = &quote.estimated_confirmation {
        println!("  ETA  : {}", conf);
    }

    // ── Execute per rail ─────────────────────────────────────────────────────
    println!();
    match &quote.selected_method {
        PaymentMethod::Lightning { .. } => {
            pay_lightning(&quote.selected_method, amount_sats, memo).await?;
        }
        PaymentMethod::Onchain { address, label, .. } => {
            pay_onchain(address, label, amount_sats)?;
        }
        PaymentMethod::Ark { server, pubkey, .. } => {
            println!("══════════════════════════════════════════════════");
            println!("  Ark payment");
            println!("══════════════════════════════════════════════════");
            println!("  Server : {}", server);
            println!("  Pubkey : {}", pubkey);
            println!();
            println!("  (Ark client not yet integrated — coming next.)");
        }
    }

    Ok(())
}

async fn pay_lightning(
    method: &PaymentMethod,
    amount_sats: u64,
    memo: Option<&str>,
) -> Result<()> {
    let ln_addr = lightning_address(method)
        .ok_or_else(|| anyhow::anyhow!("no Lightning Address in method"))?;

    println!("══════════════════════════════════════════════════");
    println!("  Lightning — {} sats to {}", amount_sats, ln_addr);
    println!("══════════════════════════════════════════════════");

    print!("  Fetching LNURL metadata... ");
    let meta = fetch_lnurl_metadata(ln_addr).await?;
    println!("ok  (range: {}–{} sats)", meta.min_sendable / 1000, meta.max_sendable / 1000);

    print!("  Requesting invoice... ");
    let invoice = fetch_invoice(&meta, amount_sats, memo).await?;
    println!("received.");

    println!();
    println!("──────────────────────────────────────────────────");
    println!("  Scan to pay");
    println!("──────────────────────────────────────────────────");
    print_qr(&invoice.to_uppercase())?;
    println!("  Amount : {} sats", amount_sats);
    if let Some(m) = memo {
        println!("  Memo   : {}", m);
    }
    println!();
    println!("  BOLT11:");
    println!("  {}", invoice);
    println!();
    println!("  ⚠  This is a real invoice. Scanning it charges real sats.");

    Ok(())
}

fn pay_onchain(address: &str, label: &str, amount_sats: u64) -> Result<()> {
    let uri = bitcoin_uri(address, amount_sats);

    println!("══════════════════════════════════════════════════");
    println!("  On-chain — {} sats", amount_sats);
    println!("══════════════════════════════════════════════════");
    println!("  Address : {} [{}]", address, label);
    println!("  BTC     : {:.8}", amount_sats as f64 / 100_000_000.0);
    println!();
    println!("──────────────────────────────────────────────────");
    println!("  Scan to pay (BIP-21 URI)");
    println!("──────────────────────────────────────────────────");
    print_qr(&uri)?;
    println!("  {}", uri);
    println!();
    println!("  ⚠  Send from your wallet to the address above.");
    println!("     No transaction was broadcast automatically.");

    Ok(())
}
