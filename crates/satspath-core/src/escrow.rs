use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    WaitingForDeposit,
    Funded,
    Claimed,
    Refunded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowRecord {
    pub escrow_id: String,
    pub receiver_alias_hash: String,
    pub amount_sats: u64,
    pub status: EscrowStatus,
    pub claim_secret_hash: String,
    pub deposit_invoice: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequest {
    pub receiver_alias_hash: String,
    pub amount_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositResponse {
    pub escrow_id: String,
    pub claim_secret: String,
    pub deposit_invoice: String,
    pub status: EscrowStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub escrow_id: String,
    pub claim_secret: String,
    pub receiver_alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResponse {
    pub status: String,
    pub message: String,
    pub amount_sats: u64,
}
