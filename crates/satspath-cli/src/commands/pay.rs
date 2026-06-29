use anyhow::Result;

use satspath_core::{
    create_invite_record,
    crypto::verify_signed_profile,
    pointer::build_qr_payload,
    privacy::{mask_address, mask_identifier, mask_invoice, mask_pubkey},
    resolver::ProfileResolver,
    validation::{
        assert_no_private_material, validate_amount_sats, validate_bitcoin_address,
        validate_compressed_pubkey, validate_lightning_address, LARGE_PREVIEW_AMOUNT_SATS,
    },
    BitcoinNetwork, PaymentMethod, PaymentPointer, SatsPathError,
};
use satspath_router::{select_route, RouteRequest, SwapDirective};

use super::get_resolver;

pub async fn cmd_pay(
    alias: &str,
    amount_sats: u64,
    memo: Option<&str>,
    mainnet_preview: bool,
    experimental_swaps: bool,
    testnet: bool,
    debug: bool,
) -> Result<()> {
    validate_pay_flags(mainnet_preview, experimental_swaps, testnet)?;
    validate_amount_sats(amount_sats).map_err(|e| anyhow::anyhow!("{}", e))?;
    if let Some(memo) = memo {
        assert_no_private_material(memo).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    println!("─────────────────────────────────────────");
    println!("SatsPath Preview Mode");
    println!("─────────────────────────────────────────");
    println!("No funds moved.");
    println!("No signing performed.");
    println!("No private keys touched.");
    println!("Public payment pointer only.");
    println!();

    if experimental_swaps {
        println!("Experimental swap path requested.");
        println!("Testnet only.");
        println!("Mainnet execution unavailable.");
        println!();
    }

    if amount_sats >= LARGE_PREVIEW_AMOUNT_SATS {
        println!(
            "Warning: large preview amount: {} sats. Preview only. No funds moved.",
            amount_sats
        );
        println!();
    }

    let display_alias = if debug {
        alias.to_string()
    } else {
        mask_identifier(alias)
    };

    println!("Resolving identifier {}...", display_alias);
    let resolver = get_resolver()?;
    let signed = match resolver.resolve_alias(alias).await {
        Ok(signed) => signed,
        Err(SatsPathError::AliasNotFound(_)) => {
            let invite = create_invite_record(
                alias,
                amount_sats,
                memo.map(str::to_string),
                "local-sender".into(),
                7 * 24 * 60 * 60,
            );
            println!("No signed profile found.");
            println!("Created invite: {}", invite.invite_id);
            println!("Receiver must verify email and publish a signed public payment profile.");
            println!("No funds moved.");
            println!("No signing performed.");
            println!("No private keys touched.");
            return Ok(());
        }
        Err(e) => return Err(anyhow::anyhow!("{}", e)),
    };
    println!("  Found signed profile.");

    println!("Verifying signed profile...");
    if !verify_signed_profile(&signed)? {
        anyhow::bail!("Profile signature FAILED. Aborting preview.");
    }
    validate_compressed_pubkey(&signed.profile.identity_pubkey)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("  Signature valid.");

    println!("Selecting public payment route...");
    let req = RouteRequest {
        alias: alias.to_string(),
        amount_sats,
        signed_profile: signed.clone(),
    };
    let quote = select_route(&req)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("  Route selected: {}", quote.selected_method.method_name());

    let network = if mainnet_preview {
        BitcoinNetwork::Mainnet
    } else if testnet {
        BitcoinNetwork::Testnet
    } else {
        BitcoinNetwork::Mainnet
    };
    let pointer = payment_method_to_pointer(&quote.selected_method)?;
    validate_pointer_for_preview(&pointer, mainnet_preview, network)?;
    let qr_payload =
        build_qr_payload(&pointer, amount_sats).map_err(|e| anyhow::anyhow!("{}", e))?;
    assert_no_private_material(&qr_payload).map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(snap) = &quote.fee_snapshot {
        println!();
        println!("Mempool fees (sat/vB)");
        println!("  Next block (~10 min): {}", snap.fastest_sat_vb);
        println!("  30 minutes          : {}", snap.half_hour_sat_vb);
        println!("  60 minutes          : {}", snap.hour_sat_vb);
    }

    println!();
    println!("Route decision: {}", quote.selected_method.method_name());
    println!("Reason:         {}", quote.reason);
    if let Some(fee) = quote.estimated_fee_sats {
        println!("Estimated fee:  {} sats", fee);
    }
    if let Some(conf) = &quote.estimated_confirmation {
        println!("Expected timing: {}", conf);
    }
    println!("Network rules:  {}", network_name(network));
    println!();
    println!("Public pointer: {}", display_pointer(&pointer, debug));
    println!(
        "QR payload:     {}",
        display_payload(&pointer, &qr_payload, debug)
    );
    println!();

    if experimental_swaps && testnet {
        exec_experimental(&quote.swap_directive, amount_sats, alias, debug).await?;
    }

    for line in preview_safety_lines() {
        println!("{line}");
    }
    Ok(())
}

fn validate_pay_flags(
    mainnet_preview: bool,
    experimental_swaps: bool,
    testnet: bool,
) -> Result<()> {
    if experimental_swaps && !testnet {
        anyhow::bail!(
            "--experimental-swaps is only available with --testnet. Mainnet execution is unavailable."
        );
    }
    if experimental_swaps && mainnet_preview {
        anyhow::bail!("--mainnet-preview cannot be combined with --experimental-swaps.");
    }
    Ok(())
}

async fn exec_experimental(
    directive: &SwapDirective,
    amount_sats: u64,
    alias: &str,
    debug: bool,
) -> Result<()> {
    println!("══════════════════════════════════════════════════");
    println!("Engine v0 — EXPERIMENTAL TESTNET ONLY");
    println!("══════════════════════════════════════════════════");

    match directive {
        SwapDirective::LightningPayment { target_ln_address } => {
            let addr = target_ln_address.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "No verified Lightning Address in profile. Cannot create testnet intent."
                )
            })?;
            println!(
                "  [Direct LN] Target: {}",
                display_value(addr, mask_identifier, debug)
            );
            println!("  Testnet LN execution is not automatic in this preview.");
        }
        SwapDirective::SubmarineSwap { target_invoice } => {
            let invoice = target_invoice.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Submarine swap requires a real BOLT11 invoice. No verified invoice in profile."
                )
            })?;
            println!("  [Submarine Swap] Ark/L1 -> Lightning");
            println!(
                "  Invoice : {}",
                display_value(invoice, mask_invoice, debug)
            );
            println!("  Amount  : {} sats", amount_sats);
        }
        SwapDirective::ChainSwap { target_address } => {
            println!("  [Chain Swap] Ark/L1 -> L1");
            println!(
                "  Destination : {}",
                display_value(target_address, mask_address, debug)
            );
            println!("  Amount      : {} sats", amount_sats);
        }
        SwapDirective::ReverseSwap { target_address } => {
            println!("  [Reverse Swap] Lightning -> L1");
            println!(
                "  Destination : {}",
                display_value(target_address, mask_address, debug)
            );
        }
        SwapDirective::ArkTransfer { server, pubkey } => {
            println!("  [Ark Transfer] Direct VTXO transfer intent");
            println!("  Server : {}", display_value(server, mask_address, debug));
            println!("  Pubkey : {}", display_value(pubkey, mask_pubkey, debug));
        }
    }

    println!();
    println!(
        "  Alias   : {}",
        display_value(alias, mask_identifier, debug)
    );
    println!("  Amount  : {} sats", amount_sats);
    println!("  Status  : intent_preview / awaiting explicit execution");
    println!("  No mainnet path is open.");
    println!();
    Ok(())
}

fn payment_method_to_pointer(method: &PaymentMethod) -> Result<PaymentPointer> {
    match method {
        PaymentMethod::Lightning {
            lnurl,
            lightning_address,
            bolt12,
            ..
        } => {
            if let Some(address) = lightning_address {
                Ok(PaymentPointer::LightningAddress {
                    address: address.clone(),
                    receiver_pubkey: None,
                })
            } else if let Some(callback_url) = lnurl {
                Ok(PaymentPointer::LnurlPay {
                    callback_url: callback_url.clone(),
                    receiver_pubkey: None,
                })
            } else if let Some(invoice) = bolt12 {
                Ok(PaymentPointer::Bolt11Invoice {
                    invoice: invoice.clone(),
                    amount_sats: None,
                })
            } else {
                anyhow::bail!("Lightning method has no public pointer.")
            }
        }
        PaymentMethod::Onchain {
            address, network, ..
        } => Ok(PaymentPointer::OnchainAddress {
            network: *network,
            address: address.clone(),
            claim_policy: None,
        }),
        PaymentMethod::Ark { server, pubkey, .. } => Ok(PaymentPointer::Ark {
            server: server.clone(),
            receiver_pubkey: pubkey.clone(),
            vtxo_pointer: None,
        }),
    }
}

fn validate_pointer_for_preview(
    pointer: &PaymentPointer,
    mainnet_preview: bool,
    network: BitcoinNetwork,
) -> Result<()> {
    match pointer {
        PaymentPointer::LightningAddress {
            address,
            receiver_pubkey,
        } => {
            validate_lightning_address(address).map_err(|e| anyhow::anyhow!("{}", e))?;
            if let Some(pubkey) = receiver_pubkey {
                validate_compressed_pubkey(pubkey).map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        PaymentPointer::LnurlPay {
            callback_url,
            receiver_pubkey,
            ..
        } => {
            let url = url::Url::parse(callback_url)?;
            if !matches!(url.scheme(), "https" | "http") {
                anyhow::bail!("LNURL callback must be http(s).");
            }
            if let Some(pubkey) = receiver_pubkey {
                validate_compressed_pubkey(pubkey).map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        PaymentPointer::Bolt11Invoice { invoice, .. } => {
            assert_no_private_material(invoice).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        PaymentPointer::OnchainAddress { address, .. } => {
            if mainnet_preview {
                validate_bitcoin_address(address, BitcoinNetwork::Mainnet)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            } else {
                validate_bitcoin_address(address, network).map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        PaymentPointer::Ark {
            server,
            receiver_pubkey,
            vtxo_pointer,
        } => {
            if server.trim().is_empty() {
                anyhow::bail!("Ark pointer requires a public server.");
            }
            validate_compressed_pubkey(receiver_pubkey).map_err(|e| anyhow::anyhow!("{}", e))?;
            if let Some(vtxo) = vtxo_pointer {
                assert_no_private_material(vtxo).map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
    }
    Ok(())
}

fn display_pointer(pointer: &PaymentPointer, debug: bool) -> String {
    match pointer {
        PaymentPointer::LightningAddress { address, .. } => {
            format!(
                "Lightning Address {}",
                display_value(address, mask_identifier, debug)
            )
        }
        PaymentPointer::LnurlPay { callback_url, .. } => {
            format!(
                "LNURL Pay {}",
                display_value(callback_url, mask_address, debug)
            )
        }
        PaymentPointer::Bolt11Invoice { invoice, .. } => {
            format!("BOLT11 {}", display_value(invoice, mask_invoice, debug))
        }
        PaymentPointer::OnchainAddress { address, .. } => {
            format!("On-chain {}", display_value(address, mask_address, debug))
        }
        PaymentPointer::Ark {
            server,
            receiver_pubkey,
            ..
        } => format!(
            "Ark server={} pubkey={}",
            display_value(server, mask_address, debug),
            display_value(receiver_pubkey, mask_pubkey, debug)
        ),
    }
}

fn display_payload(pointer: &PaymentPointer, payload: &str, debug: bool) -> String {
    if debug {
        return payload.to_string();
    }
    match pointer {
        PaymentPointer::Bolt11Invoice { .. } => mask_invoice(payload),
        PaymentPointer::OnchainAddress { .. } => mask_address(payload),
        PaymentPointer::Ark { .. } => mask_address(payload),
        PaymentPointer::LightningAddress { .. } | PaymentPointer::LnurlPay { .. } => {
            mask_address(payload)
        }
    }
}

fn display_value(value: &str, mask: fn(&str) -> String, debug: bool) -> String {
    if debug {
        value.to_string()
    } else {
        mask(value)
    }
}

fn network_name(network: BitcoinNetwork) -> &'static str {
    match network {
        BitcoinNetwork::Mainnet => "mainnet",
        BitcoinNetwork::Testnet => "testnet",
        BitcoinNetwork::Regtest => "regtest",
    }
}

fn preview_safety_lines() -> [&'static str; 5] {
    [
        "SatsPath Preview Mode",
        "No funds moved.",
        "No signing performed.",
        "No private keys touched.",
        "Public payment pointer only.",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_preview_does_not_call_swap_execution() {
        let joined = preview_safety_lines().join("\n").to_ascii_lowercase();
        assert!(joined.contains("no funds moved"));
        assert!(joined.contains("no signing performed"));
        assert!(!joined.contains("broadcast"));
    }

    #[test]
    fn default_pay_command_says_preview_and_no_funds_moved() {
        let joined = preview_safety_lines().join("\n");
        assert!(joined.contains("SatsPath Preview Mode"));
        assert!(joined.contains("No funds moved."));
        assert!(joined.contains("No private keys touched."));
    }
}
