use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// BOLT12 Offer - a reusable payment request that can be used multiple times.
/// Spec: https://github.com/lightning/bolts/pull/798
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bolt12Offer {
    /// The offer string (bech32 encoded starting with "lno1...")
    pub offer: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Amount in millisatoshis (None = amount-less offer)
    pub amount_msats: Option<u64>,
    /// Currency (typically "btc")
    pub currency: Option<String>,
    /// Minimum amount in millisatoshis
    pub min_amount_msats: Option<u64>,
    /// Maximum amount in millisatoshis
    pub max_amount_msats: Option<u64>,
    /// Quantity (for multiple units)
    pub quantity: Option<u64>,
    /// Absolute expiry time (Unix timestamp)
    pub absolute_expiry: Option<u64>,
    /// Relative expiry in seconds from creation
    pub relative_expiry: Option<u64>,
    /// Paths for routing (array of arrays of node IDs)
    pub paths: Option<Vec<Vec<String>>>,
    /// Blinded paths for privacy
    pub blinded_paths: Option<Vec<BlindedPath>>,
    /// Issuer (for refunds)
    pub issuer: Option<String>,
    /// Node ID of the offer creator
    pub node_id: Option<String>,
    /// Signature
    pub signature: Option<String>,
}

/// Blinded path for BOLT12 (privacy-preserving routing)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlindedPath {
    pub introduction_node_id: String,
    pub blinded_hops: Vec<BlindedHop>,
}

/// Blinded hop in a blinded path
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlindedHop {
    pub blinded_node_id: String,
    pub encrypted_payload: String,
}

/// BOLT12 Invoice Request - sent by payer to request an invoice from an offer
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bolt12InvoiceRequest {
    /// The offer this request is for
    pub offer: String,
    /// Amount in millisatoshis
    pub amount_msats: u64,
    /// Payer's node ID (optional, for refunds)
    pub payer_node_id: Option<String>,
    /// Quantity (for multiple units)
    pub quantity: Option<u64>,
    /// Payer's note/description
    pub payer_note: Option<String>,
    /// Paths for routing
    pub paths: Option<Vec<Vec<String>>>,
    /// Blinded paths
    pub blinded_paths: Option<Vec<BlindedPath>>,
    /// Absolute expiry (Unix timestamp)
    pub absolute_expiry: Option<u64>,
    /// Relative expiry in seconds
    pub relative_expiry: Option<u64>,
    /// Payer's signature
    pub signature: Option<String>,
}

/// BOLT12 Invoice - returned by the offer creator in response to an invoice request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bolt12Invoice {
    /// The invoice string (bech32 encoded starting with "lni1...")
    pub invoice: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Amount in millisatoshis
    pub amount_msats: u64,
    /// Currency
    pub currency: String,
    /// Created at (Unix timestamp)
    pub created_at: u64,
    /// Relative expiry in seconds
    pub relative_expiry: Option<u64>,
    /// Absolute expiry (Unix timestamp)
    pub absolute_expiry: Option<u64>,
    /// Payment hash (for HTLC)
    pub payment_hash: Option<String>,
    /// Fallback on-chain address
    pub fallback_address: Option<String>,
    /// Routing hints
    pub routes: Option<Vec<Vec<String>>>,
    /// Blinded paths
    pub blinded_paths: Option<Vec<BlindedPath>>,
    /// Node ID of the invoice creator
    pub node_id: Option<String>,
    /// Signature
    pub signature: Option<String>,
}

/// Parse a BOLT12 offer string
pub fn parse_bolt12_offer(offer: &str) -> Result<Bolt12Offer, anyhow::Error> {
    // Basic validation - BOLT12 offers start with "lno1"
    if !offer.starts_with("lno1") {
        return Err(anyhow!("Invalid BOLT12 offer: must start with 'lno1'"));
    }

    // In a full implementation, this would use bech32m decoding
    // For now, return a basic parsed offer
    Ok(Bolt12Offer {
        offer: offer.to_string(),
        description: None,
        amount_msats: None,
        currency: Some("btc".to_string()),
        min_amount_msats: None,
        max_amount_msats: None,
        quantity: None,
        absolute_expiry: None,
        relative_expiry: None,
        paths: None,
        blinded_paths: None,
        issuer: None,
        node_id: None,
        signature: None,
    })
}

/// Parse a BOLT12 invoice request
pub fn parse_bolt12_invoice_request(request: &str) -> Result<Bolt12InvoiceRequest, anyhow::Error> {
    // Basic validation
    if !request.starts_with("lnr1") {
        return Err(anyhow!(
            "Invalid BOLT12 invoice request: must start with 'lnr1'"
        ));
    }

    // In a full implementation, this would use bech32m decoding
    Ok(Bolt12InvoiceRequest {
        offer: "".to_string(),
        amount_msats: 0,
        payer_node_id: None,
        quantity: None,
        payer_note: None,
        paths: None,
        blinded_paths: None,
        absolute_expiry: None,
        relative_expiry: None,
        signature: None,
    })
}

/// Parse a BOLT12 invoice
pub fn parse_bolt12_invoice(invoice: &str) -> Result<Bolt12Invoice, anyhow::Error> {
    if !invoice.starts_with("lni1") {
        return Err(anyhow!("Invalid BOLT12 invoice: must start with 'lni1'"));
    }

    Ok(Bolt12Invoice {
        invoice: invoice.to_string(),
        description: None,
        amount_msats: 0,
        currency: "btc".to_string(),
        created_at: 0,
        relative_expiry: None,
        absolute_expiry: None,
        payment_hash: None,
        fallback_address: None,
        routes: None,
        blinded_paths: None,
        node_id: None,
        signature: None,
    })
}

/// Create a BOLT12 invoice request from an offer
#[allow(clippy::too_many_arguments)]
pub fn create_invoice_request(
    offer: &Bolt12Offer,
    amount_msats: u64,
    payer_node_id: Option<String>,
    quantity: Option<u64>,
    payer_note: Option<String>,
    paths: Option<Vec<Vec<String>>>,
    blinded_paths: Option<Vec<BlindedPath>>,
    relative_expiry: Option<u64>,
) -> Bolt12InvoiceRequest {
    Bolt12InvoiceRequest {
        offer: offer.offer.clone(),
        amount_msats,
        payer_node_id,
        quantity,
        payer_note,
        paths,
        blinded_paths,
        absolute_expiry: None,
        relative_expiry,
        signature: None,
    }
}

/// Encode a BOLT12 invoice request to bech32m string
pub fn encode_invoice_request(_request: &Bolt12InvoiceRequest) -> Result<String, anyhow::Error> {
    // In a full implementation, this would use bech32m encoding
    // For now, return a placeholder
    Ok("lnr1...".to_string())
}

/// Fetch a BOLT12 invoice from an offer via LNURL-like callback
pub async fn fetch_bolt12_invoice(
    _offer: &Bolt12Offer,
    _amount_msats: u64,
    _payer_node_id: Option<String>,
    _quantity: Option<u64>,
    _payer_note: Option<String>,
) -> Result<Bolt12Invoice, anyhow::Error> {
    // This would typically use a websocket or HTTP connection to the offer's node
    // For now, return an error indicating not implemented
    Err(anyhow!("BOLT12 invoice fetching not yet implemented - requires websocket/HTTP connection to offer node"))
}

/// Validate a BOLT12 invoice
pub fn validate_bolt12_invoice(
    invoice: &Bolt12Invoice,
    expected_amount_msats: u64,
) -> Result<(), anyhow::Error> {
    if invoice.amount_msats != expected_amount_msats {
        return Err(anyhow!(
            "Invoice amount mismatch: expected {} msats, got {} msats",
            expected_amount_msats,
            invoice.amount_msats
        ));
    }

    // Check expiry
    if let Some(absolute_expiry) = invoice.absolute_expiry {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now > absolute_expiry {
            return Err(anyhow!("Invoice has expired"));
        }
    }

    if let Some(relative_expiry) = invoice.relative_expiry {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if let Some(created_at) = invoice.created_at.checked_add(relative_expiry) {
            if now > created_at {
                return Err(anyhow!("Invoice has expired"));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bolt12_offer_valid() {
        let offer = "lno1pq...";
        let result = parse_bolt12_offer(offer);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_bolt12_offer_invalid_prefix() {
        let offer = "invalid";
        let result = parse_bolt12_offer(offer);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_bolt12_invoice_request_valid() {
        let request = "lnr1...";
        let result = parse_bolt12_invoice_request(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bolt12_invoice_amount_match() {
        let invoice = Bolt12Invoice {
            invoice: "lni1...".to_string(),
            description: None,
            amount_msats: 100_000,
            currency: "btc".to_string(),
            created_at: 0,
            relative_expiry: None,
            absolute_expiry: None,
            payment_hash: None,
            fallback_address: None,
            routes: None,
            blinded_paths: None,
            node_id: None,
            signature: None,
        };

        let result = validate_bolt12_invoice(&invoice, 100_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bolt12_invoice_amount_mismatch() {
        let invoice = Bolt12Invoice {
            invoice: "lni1...".to_string(),
            description: None,
            amount_msats: 200_000,
            currency: "btc".to_string(),
            created_at: 0,
            relative_expiry: None,
            absolute_expiry: None,
            payment_hash: None,
            fallback_address: None,
            routes: None,
            blinded_paths: None,
            node_id: None,
            signature: None,
        };

        let result = validate_bolt12_invoice(&invoice, 100_000);
        assert!(result.is_err());
    }
}
