use std::time::Duration;
use anyhow::Result;

use satspath_core::{
    crypto::verify_signed_profile,
    PaymentMethod,
};
use satspath_router::{select_route, RouteRequest, SwapDirective};
use satspath_swaps::{
    boltz_client::BoltzClient,
    chain_swap::ChainSwapParams,
    submarine::SubmarineParams,
    swap_manager::SwapManager,
    swap_store::SwapStore,
    ark_bridge::ArkBridge,
};

use super::open_registry;

pub async fn cmd_pay(alias: &str, amount_sats: u64) -> Result<()> {
    println!("─────────────────────────────────────────");
    println!("SatsPath Payment Engine");
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
    println!("Initializing Swap Engine (Testnet)...");
    
    // Initialize swaps
    let client = BoltzClient::testnet();
    let store = SwapStore::open().unwrap_or_else(|_| SwapStore::open_plaintext(std::env::current_dir().unwrap().join(".satspath/swaps.json")));
    
    // Attempt to spawn ARK bridge (will fail gracefully if not built yet)
    let manager = if let Ok(bridge) = ArkBridge::spawn(std::path::PathBuf::from("../../ark-bridge")) {
        println!("  ARK Bridge connected.");
        SwapManager::new(client, store).with_ark_bridge(bridge)
    } else {
        println!("  Warning: ARK Bridge not found. VTXO validation will be skipped.");
        SwapManager::new(client, store)
    };

    println!("Executing payment of {} sats to {}...", amount_sats, alias);

    // Execute payment based on SwapDirective
    match &quote.swap_directive {
        SwapDirective::SubmarineSwap { target_invoice } => {
            println!("  [Submarine Swap] Ark/L1 → Lightning");
            let invoice = target_invoice.clone().unwrap_or_else(|| "lnbc100...".to_string());
            
            println!("  Requesting Submarine Swap from Boltz...");
            let created = manager.create_submarine(SubmarineParams {
                invoice,
                amount_sats,
            }).await?;
            
            println!("  Swap Created: {}", created.swap_id);
            println!("  ACTION REQUIRED: Send {} sats to {}", created.expected_amount_sats, created.lockup_address);
            println!("  Waiting for Boltz to pay the invoice (timeout 2 min)...");
            
            match manager.wait_and_claim(&created.swap_id, Duration::from_secs(120)).await {
                Ok(res) => println!("  Success! Payment complete. Status: {:?}", res.status),
                Err(e) => println!("  Swap Failed: {}", e),
            }
        }
        SwapDirective::ChainSwap { target_address } => {
            println!("  [Chain Swap] Ark/L1 → L1");
            println!("  Requesting Chain Swap from Boltz to {}...", target_address);
            
            let created = manager.create_chain_swap(ChainSwapParams {
                send_amount_sats: amount_sats,
                destination_address: target_address.clone(),
                sender_pays_fees: true,
            }).await?;

            println!("  Swap Created: {}", created.swap_id);
            println!("  ACTION REQUIRED: Send {} sats to {}", created.lock_amount_sats, created.sender_lockup_address);
            println!("  Waiting for both lockups to confirm, then claiming (timeout 5 min)...");
            
            match manager.wait_and_claim(&created.swap_id, Duration::from_secs(300)).await {
                Ok(res) => println!("  Success! Funds claimed to {}. TXID: {:?}", target_address, res.settlement_txid),
                Err(e) => println!("  Swap Failed: {}", e),
            }
        }
        SwapDirective::LightningPayment { target_invoice: _ } => {
            println!("  [Direct LN] Simulation only: LN node integration pending.");
        }
        SwapDirective::ReverseSwap { target_address: _ } => {
            println!("  [Reverse Swap] Lightning → L1 (Not triggerable from sender side directly in MVP)");
        }
        SwapDirective::ArkTransfer { server, pubkey } => {
            println!("  [Ark Transfer] Direct transfer within Ark server {}", server);
            println!("  Simulation only: Virtual UTXO creation for {} pending.", pubkey);
        }
    }

    println!();
    println!("Payment status: processing / complete");
    Ok(())
}
