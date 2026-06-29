use anyhow::Result;
use qrcode::render::unicode;
use qrcode::QrCode;

use satspath_core::{crypto::verify_signed_profile, PaymentMethod};
use satspath_router::{
    fetch_invoice, fetch_lnurl_metadata, lightning::lightning_address, select_route, RouteRequest,
};

use super::open_registry;

pub async fn cmd_pay(alias: &str, amount_sats: u64, memo: Option<&str>) -> Result<()> {
    separator();
    println!("  SatsPath — Live Payment");
    separator();

    // ── 1. Resolve ──────────────────────────────────────────────────────────
    print!("Resolving {}... ", alias);
    let registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("found.");

    // ── 2. Verify signature ─────────────────────────────────────────────────
    print!("Verifying profile signature... ");
    if !verify_signed_profile(signed)? {
        anyhow::bail!("signature INVALID — profile may be tampered. Aborting.");
    }
    println!("valid.");

    // ── 3. Route selection ──────────────────────────────────────────────────
    println!("Selecting payment rail for {} sats...", amount_sats);
    let req = RouteRequest {
        alias: alias.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let quote = select_route(&req)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!();
    println!("  Rail selected : {}", quote.selected_method.method_name());
    println!("  Reason        : {}", quote.reason);
    if let Some(fee) = quote.estimated_fee_sats {
        println!("  Estimated fee : {} sats", fee);
    }
    if let Some(conf) = &quote.estimated_confirmation {
        println!("  Confirmation  : {}", conf);
    }

    // ── 4. Execute against selected rail ────────────────────────────────────
    println!();
    match &quote.selected_method {
        PaymentMethod::Lightning { .. } => {
            pay_via_lightning(&quote.selected_method, amount_sats, memo).await?;
        }
        PaymentMethod::Onchain { address, label, .. } => {
            println!("On-chain payment to: {} [{}]", address, label);
            println!();
            println!("Scan address QR:");
            print_qr(address)?;
            println!();
            println!("DISCLAIMER: No real transaction was broadcast. Send manually from your wallet.");
        }
        PaymentMethod::Ark { server, pubkey, .. } => {
            println!("Ark payment via server: {}", server);
            println!("Pubkey: {}", pubkey);
            println!("(Ark integration not yet live — coming next.)");
        }
    }

    Ok(())
}

async fn pay_via_lightning(
    method: &PaymentMethod,
    amount_sats: u64,
    memo: Option<&str>,
) -> Result<()> {
    let ln_addr = lightning_address(method)
        .ok_or_else(|| anyhow::anyhow!("no Lightning Address in method"))?;

    println!("Lightning Address : {}", ln_addr);

    // Step 1: fetch LNURL metadata
    print!("Fetching LNURL metadata... ");
    let meta = fetch_lnurl_metadata(ln_addr).await?;
    println!("ok.");
    println!(
        "  Range: {} – {} sats",
        meta.min_sendable / 1000,
        meta.max_sendable / 1000
    );

    // Step 2: fetch real BOLT11 invoice
    print!("Requesting invoice for {} sats... ", amount_sats);
    let invoice = fetch_invoice(&meta, amount_sats, memo).await?;
    println!("received.");

    // Step 3: display
    println!();
    thin_separator();
    println!("  BOLT11 Invoice");
    thin_separator();
    println!("{}", invoice);
    println!();
    thin_separator();
    println!("  Scan QR to pay");
    thin_separator();
    // BOLT11 must be uppercase for QR alphanumeric mode (smaller QR)
    print_qr(&invoice.to_uppercase())?;
    println!();
    println!("Open any Lightning wallet and scan the QR above.");
    println!("Amount : {} sats", amount_sats);
    if let Some(m) = memo {
        println!("Memo   : {}", m);
    }
    println!();
    println!("NOTE: This is a real invoice. Scanning it will charge real sats.");

    Ok(())
}

fn print_qr(data: &str) -> Result<()> {
    let code = QrCode::new(data.as_bytes())
        .map_err(|e| anyhow::anyhow!("QR encode error: {}", e))?;
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .quiet_zone(true)
        .build();
    println!("{}", image);
    Ok(())
}

fn separator() {
    println!("══════════════════════════════════════════════════");
}

fn thin_separator() {
    println!("──────────────────────────────────────────────────");
}
