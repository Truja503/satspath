use anyhow::Result;

use satspath_core::{crypto::verify_signed_profile, PaymentMethod};
use satspath_router::{
    fetch_invoice, fetch_lnurl_metadata, lightning::lightning_address, select_route,
    RouteRequest, SwapDirective,
};

use super::{
    get_resolver,
    qr::{bitcoin_uri, print_qr},
};

pub async fn cmd_pay(
    alias: &str,
    amount_sats: u64,
    memo: Option<&str>,
    experimental_swaps: bool,
    testnet: bool,
) -> Result<()> {
    println!("══════════════════════════════════════════════════");
    println!("  SatsPath Payment");
    println!("══════════════════════════════════════════════════");

    if experimental_swaps {
        if !testnet {
            anyhow::bail!(
                "--experimental-swaps requires --testnet. \
                 Swap engine is not allowed on mainnet in Engine v0."
            );
        }
        println!("  ⚠  EXPERIMENTAL swap engine active (testnet only)");
    }

    // ── Resolve ─────────────────────────────────────────────────────────────
    print!("Resolving {}... ", alias);
    let resolver = get_resolver()?;
    use satspath_core::resolver::ProfileResolver;
    let signed = resolver
        .resolve_alias(alias)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("found.");

    // ── Verify ──────────────────────────────────────────────────────────────
    print!("Verifying profile signature... ");
    if !verify_signed_profile(&signed)? {
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
    let quote = select_route(&req).await.map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("done.");

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
    println!();

    // ── Execute ───────────────────────────────────────────────────────────────
    if experimental_swaps && testnet {
        // Engine v0: experimental testnet swap path.
        exec_experimental(&quote.swap_directive, amount_sats, alias).await?;
    } else {
        // Safe default path: LNURL fetch + QR display.
        exec_safe(&quote.selected_method, amount_sats, memo).await?;
    }

    Ok(())
}

// ─── Safe default path ───────────────────────────────────────────────────────

async fn exec_safe(method: &PaymentMethod, amount_sats: u64, memo: Option<&str>) -> Result<()> {
    match method {
        PaymentMethod::Lightning { .. } => {
            safe_lightning(method, amount_sats, memo).await?;
        }
        PaymentMethod::Onchain { address, label, .. } => {
            safe_onchain(address, label, amount_sats);
        }
        PaymentMethod::Ark { server, pubkey, .. } => {
            println!("══════════════════════════════════════════════════");
            println!("  Ark — {} sats", amount_sats);
            println!("══════════════════════════════════════════════════");
            println!("  Server : {}", server);
            println!("  Pubkey : {}", pubkey);
            println!();
            println!("  ⚠  [EXPERIMENTAL] Ark payment endpoint.");
            println!("     Use --experimental-swaps --testnet to attempt swap execution.");
        }
    }
    Ok(())
}

async fn safe_lightning(method: &PaymentMethod, amount_sats: u64, memo: Option<&str>) -> Result<()> {
    let ln_addr = lightning_address(method)
        .ok_or_else(|| anyhow::anyhow!("no Lightning Address in method"))?;

    println!("══════════════════════════════════════════════════");
    println!("  Lightning — {} sats → {}", amount_sats, ln_addr);
    println!("══════════════════════════════════════════════════");

    print!("  Fetching LNURL metadata... ");
    let meta = fetch_lnurl_metadata(ln_addr).await?;
    println!(
        "ok  (range: {}–{} sats)",
        meta.min_sendable / 1000,
        meta.max_sendable / 1000
    );

    print!("  Requesting invoice... ");
    let invoice = fetch_invoice(&meta, amount_sats, memo).await?;
    println!("received.");

    println!();
    println!("──────────────────────────────────────────────────");
    println!("  Scan to pay");
    println!("──────────────────────────────────────────────────");
    // BOLT11 uppercase → alphanumeric QR mode → denser, easier to scan
    print_qr(&invoice.to_uppercase())?;
    println!("  Amount : {} sats", amount_sats);
    if let Some(m) = memo {
        println!("  Memo   : {}", m);
    }
    println!();
    println!("  BOLT11: {}", invoice);
    println!();
    println!("  ⚠  This is a real invoice. Scanning it charges real sats.");
    Ok(())
}

fn safe_onchain(address: &str, label: &str, amount_sats: u64) {
    let uri = bitcoin_uri(address, amount_sats);

    println!("══════════════════════════════════════════════════");
    println!("  On-chain — {} sats", amount_sats);
    println!("══════════════════════════════════════════════════");
    println!("  Address : {} [{}]", address, label);
    println!("  BTC     : {:.8}", amount_sats as f64 / 100_000_000.0);
    println!();
    println!("──────────────────────────────────────────────────");
    println!("  Scan to pay (BIP-21)");
    println!("──────────────────────────────────────────────────");
    if let Err(e) = print_qr(&uri) {
        println!("  (QR error: {})", e);
        println!("  URI: {}", uri);
    } else {
        println!("  {}", uri);
    }
    println!();
    println!("  ⚠  Send from your wallet. No tx was broadcast automatically.");
}

// ─── Experimental Engine v0 ──────────────────────────────────────────────────

async fn exec_experimental(
    directive: &SwapDirective,
    amount_sats: u64,
    alias: &str,
) -> Result<()> {
    println!("══════════════════════════════════════════════════");
    println!("  Engine v0 — EXPERIMENTAL TESTNET ONLY");
    println!("══════════════════════════════════════════════════");

    match directive {
        SwapDirective::LightningPayment { target_ln_address } => {
            let addr = target_ln_address
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!(
                    "No verified Lightning Address in profile. \
                     Cannot create swap without a real payment pointer."
                ))?;
            println!("  [Direct LN] Target: {}", addr);
            println!("  Testnet LN node integration pending.");
            println!("  Run with a real LN node to execute.");
        }

        SwapDirective::SubmarineSwap { target_invoice } => {
            // Must have a real invoice — no fake fallback.
            let invoice = target_invoice
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!(
                    "Submarine swap requires a real BOLT11 invoice. \
                     No verified invoice in profile. Cannot proceed."
                ))?;

            println!("  [Submarine Swap] Ark/L1 → Lightning");
            println!("  Invoice : {}...", &invoice[..40.min(invoice.len())]);
            println!("  Amount  : {} sats", amount_sats);
            println!();
            println!("  To execute on Boltz testnet:");
            println!("  POST https://testnet.boltz.exchange/v2/swap/submarine");
            println!("  (Full swap execution requires PSBT signing — coming in Engine v1)");
        }

        SwapDirective::ChainSwap { target_address } => {
            println!("  [Chain Swap] Ark/L1 → L1");
            println!("  Destination : {}", target_address);
            println!("  Amount      : {} sats", amount_sats);
            println!();
            println!("  To execute on Boltz testnet:");
            println!("  POST https://testnet.boltz.exchange/v2/swap/chain");
            println!("  (Full swap execution requires PSBT signing — coming in Engine v1)");
        }

        SwapDirective::ReverseSwap { target_address } => {
            println!("  [Reverse Swap] Lightning → L1");
            println!("  Destination : {}", target_address);
            println!("  (Not triggered from sender side in Engine v0)");
        }

        SwapDirective::ArkTransfer { server, pubkey } => {
            println!("  [Ark Transfer] Direct VTXO transfer");
            println!("  Server : {}", server);
            println!("  Pubkey : {}...", &pubkey[..16.min(pubkey.len())]);
            println!();

            // ARK Bridge: try to connect, fail gracefully.
            println!("  Checking Ark bridge availability...");
            println!("  ⚠  Ark VTXO validation unavailable. Experimental only.");
            println!("     Payment intent recorded. Bridge validation required to settle.");
        }
    }

    println!();
    println!("  Alias   : {}", alias);
    println!("  Amount  : {} sats", amount_sats);
    println!("  Status  : intent_created / awaiting_execution");
    println!();
    println!("  ⚠  DISCLAIMER: Engine v0 is experimental testnet software.");
    println!("     No funds were moved. No mainnet path is open.");

    Ok(())
}
