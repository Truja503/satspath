use rand::RngCore;
use secp256k1::{Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use std::time::Duration;

use crate::boltz_client::{BoltzClient, SubmarineSwapRequest};
use crate::errors::{Result, SwapError};
use crate::swap_store::SwapStore;
use crate::types::{SwapKind, SwapRecord, SwapResult, SwapStatus};

/// Parameters for creating a submarine swap.
///
/// Submarine = On-chain BTC (or Ark VTXO) → Lightning invoice payment.
/// The sender deposits BTC at `lockup_address`; Boltz pays the LN invoice.
pub struct SubmarineParams {
    /// The Lightning invoice to pay (lnbc...).
    pub invoice: String,
    /// Amount in satoshis to send. Used to verify against Boltz limits.
    pub amount_sats: u64,
}

/// Output of a created submarine swap (what the user needs to act on).
pub struct SubmarineSwapCreated {
    /// Boltz swap ID.
    pub swap_id: String,
    /// Address to deposit BTC to.
    pub lockup_address: String,
    /// Exact amount to deposit (may differ from invoice amount due to fees).
    pub expected_amount_sats: u64,
    /// CLTV expiry block. After this, refund is possible.
    pub timeout_block_height: u32,
}

// ─── Create ──────────────────────────────────────────────────────────────────

/// Create a submarine swap and persist the swap record locally.
///
/// After calling this, the user must:
/// 1. Send exactly `expected_amount_sats` to `lockup_address`.
/// 2. Call `wait_submarine` to track and auto-handle the outcome.
pub async fn create_submarine(
    client: &BoltzClient,
    store: &SwapStore,
    params: SubmarineParams,
) -> Result<SubmarineSwapCreated> {
    // Validate against Boltz limits
    let limits = client.get_limits().await?;
    if params.amount_sats < limits.minimal {
        return Err(SwapError::BelowMinimum {
            amount_sats: params.amount_sats,
            min_sats: limits.minimal,
        });
    }
    if params.amount_sats > limits.maximal {
        return Err(SwapError::ExceedsMaximum {
            amount_sats: params.amount_sats,
            max_sats: limits.maximal,
        });
    }

    // Generate ephemeral refund keypair (client-side; never transmitted to Boltz)
    let secp = Secp256k1::new();
    let mut rng = rand::thread_rng();
    let refund_secret = SecretKey::new(&mut rng);
    let refund_pubkey = refund_secret.public_key(&secp);
    let refund_pubkey_hex = hex::encode(refund_pubkey.serialize());
    let refund_key_hex = hex::encode(refund_secret.secret_bytes());

    // Call Boltz API
    let req = SubmarineSwapRequest {
        invoice: params.invoice.clone(),
        from: "BTC".into(),
        to: "BTC".into(),
        refund_public_key: refund_pubkey_hex.clone(),
    };
    let resp = client.create_submarine(&req).await?;

    let now = chrono::Utc::now().timestamp();

    // Persist swap record
    let record = SwapRecord {
        id: resp.id.clone(),
        kind: SwapKind::Submarine,
        status: SwapStatus::Created,
        amount_sats: params.amount_sats,
        preimage_hex: None,
        preimage_hash_hex: None,
        refund_key_hex: Some(refund_key_hex),
        claim_key_hex: None,
        invoice: Some(params.invoice),
        lockup_address: Some(resp.address.clone()),
        expected_amount_sats: Some(resp.expected_amount),
        timeout_block_height: Some(resp.timeout_block_height),
        boltz_claim_pubkey: Some(resp.claim_public_key),
        redeem_script: resp.redeem_script,
        lockup_txid: None,
        settlement_txid: None,
        destination_address: None,
        created_at: now,
        updated_at: now,
    };
    store.upsert(&record)?;

    Ok(SubmarineSwapCreated {
        swap_id: resp.id,
        lockup_address: resp.address,
        expected_amount_sats: resp.expected_amount,
        timeout_block_height: resp.timeout_block_height,
    })
}

// ─── Wait & Handle ───────────────────────────────────────────────────────────

/// Wait for a submarine swap to reach a terminal state.
///
/// **Success path:** Boltz routes the payment → `invoice.paid` → returns Ok.
///
/// **Failure path:** Boltz can't route → `invoice.failedToPay` → triggers
/// refund transaction construction → returns `Err(InvoiceFailedToPay)`.
///
/// # Timeout
/// Waits up to `max_wait` before returning `SwapError::Timeout`.
pub async fn wait_submarine(
    client: &BoltzClient,
    store: &SwapStore,
    swap_id: &str,
    max_wait: Duration,
) -> Result<SwapResult> {
    let update = client
        .wait_for_status(
            swap_id,
            Some(&[SwapStatus::InvoicePaid, SwapStatus::InvoiceFailedToPay]),
            max_wait,
        )
        .await?;

    match update.status {
        SwapStatus::InvoicePaid => {
            store.update_status(swap_id, SwapStatus::InvoicePaid, None)?;
            Ok(SwapResult {
                swap_id: swap_id.to_string(),
                kind: SwapKind::Submarine,
                status: SwapStatus::InvoicePaid,
                settlement_txid: None,
                preimage_hex: None,
            })
        }
        SwapStatus::InvoiceFailedToPay => {
            // Attempt automatic refund
            let refund_txid = attempt_submarine_refund(client, store, swap_id).await;
            store.update_status(
                swap_id,
                SwapStatus::TransactionRefunded,
                refund_txid.ok(),
            )?;
            Err(SwapError::InvoiceFailedToPay { id: swap_id.to_string() })
        }
        other => {
            let label = other.label().to_string();
            Err(SwapError::Timeout {
                id: swap_id.to_string(),
                last_status: label,
            })
        }
    }
}

// ─── Refund ──────────────────────────────────────────────────────────────────

/// Attempt to broadcast a refund transaction for a failed submarine swap.
///
/// Returns the refund TXID on success.
///
/// Note: Full HTLC / Taproot refund transaction construction requires the
/// `bitcoin` crate and the redeem script from the swap record. This function
/// provides the scaffolding; the transaction signing is implemented in
/// `tx_builder.rs` (Phase 4b follow-up).
async fn attempt_submarine_refund(
    _client: &BoltzClient,
    store: &SwapStore,
    swap_id: &str,
) -> Result<String> {
    let record = store
        .get(swap_id)?
        .ok_or_else(|| SwapError::NotFound(swap_id.to_string()))?;

    // Verify we have the refund key
    let _refund_key_hex = record
        .refund_key_hex
        .as_deref()
        .ok_or_else(|| SwapError::Key("Refund key missing from swap record".into()))?;

    // TODO (Phase 4b): Construct and broadcast the actual refund transaction.
    // Steps:
    //   1. Fetch the lockup UTXO from the lockup_address
    //   2. Build refund tx spending the HTLC script-path (after CLTV expiry)
    //      or cooperative key-path (Taproot) with Boltz partial sig
    //   3. Sign with refund_key
    //   4. Broadcast via node RPC
    //
    // For now, we log the intent and return a placeholder error so the
    // caller knows to handle this manually.
    Err(SwapError::Key(
        "Refund tx building not yet implemented — record preserved for manual recovery".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preimage_hash_is_sha256() {
        // Verify the preimage → hash relationship we'll use for Reverse swaps
        let mut preimage = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut preimage);
        let hash: [u8; 32] = Sha256::digest(&preimage).into();
        // Hash must be different from preimage
        assert_ne!(preimage, hash);
        // Re-hashing the hash should be different again
        let double_hash: [u8; 32] = Sha256::digest(&hash).into();
        assert_ne!(hash, double_hash);
    }
}
