use anyhow::Result;

use satspath_core::{
    crypto::verify_signed_profile,
    pointer::build_qr_payload,
    privacy::{mask_address, mask_identifier, mask_invoice, mask_pubkey},
    validation::{
        assert_no_private_material, validate_amount_sats, validate_bitcoin_address,
        validate_compressed_pubkey, validate_lightning_address, LARGE_PREVIEW_AMOUNT_SATS,
    },
    BitcoinNetwork, PaymentMethod, PaymentPointer,
};
use satspath_router::{select_route, RouteRequest};

use super::open_registry;

pub async fn cmd_pay(
    alias: &str,
    amount_sats: u64,
    mainnet_preview: bool,
    experimental_swaps: bool,
    testnet: bool,
    debug: bool,
) -> Result<()> {
    validate_pay_flags(mainnet_preview, experimental_swaps, testnet)?;
    validate_amount_sats(amount_sats).map_err(|e| anyhow::anyhow!("{}", e))?;

    if experimental_swaps {
        return cmd_experimental_testnet_swap_preview(alias, amount_sats, debug).await;
    }

    println!("─────────────────────────────────────────");
    println!("SatsPath Preview Mode");
    println!("─────────────────────────────────────────");
    println!("No funds moved.");
    println!("No signing performed.");
    println!("No private keys touched.");
    println!("Public payment pointer only.");
    println!();

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
    let registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("  Found signed profile.");

    println!("Verifying signed profile...");
    if !verify_signed_profile(signed)? {
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

    let network = if mainnet_preview {
        BitcoinNetwork::Mainnet
    } else if testnet {
        BitcoinNetwork::Testnet
    } else {
        BitcoinNetwork::Mainnet
    };
    let pointer = payment_method_to_pointer(&quote.selected_method, network)?;
    validate_pointer_for_preview(&pointer, mainnet_preview, network)?;
    let qr_payload =
        build_qr_payload(&pointer, amount_sats).map_err(|e| anyhow::anyhow!("{}", e))?;
    assert_no_private_material(&qr_payload).map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("  Route selected: {}", quote.selected_method.method_name());
    println!();
    println!("Route decision: {}", quote.selected_method.method_name());
    println!("Reason:         {}", quote.reason);
    if let Some(fee) = quote.estimated_fee_sats {
        println!("Estimated fee:  {} sats", fee);
    }
    if let Some(conf) = quote.estimated_confirmation {
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

async fn cmd_experimental_testnet_swap_preview(
    alias: &str,
    amount_sats: u64,
    debug: bool,
) -> Result<()> {
    let display_alias = if debug {
        alias.to_string()
    } else {
        mask_identifier(alias)
    };
    println!("SatsPath experimental testnet swap mode");
    println!("Identifier: {}", display_alias);
    println!("Amount:     {} sats", amount_sats);
    println!("Testnet only.");
    println!("No mainnet execution available.");
    println!("No funds moved.");
    Ok(())
}

fn payment_method_to_pointer(
    method: &PaymentMethod,
    network: BitcoinNetwork,
) -> Result<PaymentPointer> {
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
                    metadata_hash: None,
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
            address,
            pubkey_hint,
            ..
        } => Ok(PaymentPointer::OnchainAddress {
            network,
            address: address.clone(),
            derivation_hint: pubkey_hint.clone(),
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
        assert!(!joined.contains("swap execution"));
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
