use std::path::PathBuf;

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};

use crate::errors::{Result, SwapError};
use crate::types::{SwapKind, SwapRecord, SwapStatus};

/// Container for the swap store file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SwapStoreFile {
    swaps: Vec<SwapRecord>,
}

/// Persistent, encrypted local store for in-progress swap records.
///
/// Stored at `.satspath/swaps.enc` (AES-256-GCM encrypted JSON).
/// Plaintext storage is available only through explicit test/dev APIs.
pub struct SwapStore {
    path: PathBuf,
    encryption_key: Option<[u8; 32]>,
    allow_plaintext: bool,
}

impl SwapStore {
    /// Open the swap store at the default location (`.satspath/swaps.enc`).
    pub fn open() -> Result<Self> {
        Err(SwapError::Encryption(
            "SwapStore::open requires an encryption key; use open_encrypted(key)".into(),
        ))
    }

    /// Open the store with an AES-256 encryption key derived from user password.
    /// The key should be derived via PBKDF2 (same scheme as ARK SDK's StorageCrypto).
    pub fn open_encrypted(key: [u8; 32]) -> Result<Self> {
        let path = satspath_dir()?.join("swaps.enc");
        Ok(Self {
            path,
            encryption_key: Some(key),
            allow_plaintext: false,
        })
    }

    /// Open a plaintext store at a custom path (for tests / dev).
    /// Sensitive records (preimage_hex, refund_key_hex, claim_key_hex) are still rejected.
    pub fn open_plaintext(path: PathBuf) -> Self {
        Self {
            path,
            encryption_key: None,
            allow_plaintext: true,
        }
    }

    // ── Read ─────────────────────────────────────────────────────────────────

    fn load_all(&self) -> Result<SwapStoreFile> {
        if !self.path.exists() {
            return Ok(SwapStoreFile::default());
        }

        let raw = std::fs::read(&self.path)?;

        if raw.is_empty() {
            return Ok(SwapStoreFile::default());
        }

        let json_bytes = if self.allow_plaintext {
            raw
        } else if let Some(key) = &self.encryption_key {
            decrypt(key, &raw)?
        } else {
            return Err(SwapError::Encryption(
                "refusing to read swap store without encryption key".into(),
            ));
        };

        serde_json::from_slice(&json_bytes)
            .map_err(|e| SwapError::StorageCorruption(format!("Failed to parse swap store: {e}")))
    }

    // ── Write ────────────────────────────────────────────────────────────────

    fn save_all(&self, store: &SwapStoreFile) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_vec_pretty(store).map_err(SwapError::Json)?;

        let to_write = if self.allow_plaintext {
            json
        } else if let Some(key) = &self.encryption_key {
            encrypt(key, &json)?
        } else {
            return Err(SwapError::Encryption(
                "refusing to write swap store without encryption key".into(),
            ));
        };

        std::fs::write(&self.path, to_write)?;
        Ok(())
    }

    // ── Sensitive material guard ──────────────────────────────────────────────

    /// Returns true if the record contains cryptographic secrets that must not
    /// be stored in plaintext: preimage, refund key, or claim key.
    pub fn contains_sensitive_material(record: &SwapRecord) -> bool {
        record.preimage_hex.is_some()
            || record.refund_key_hex.is_some()
            || record.claim_key_hex.is_some()
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Save a new swap record. Overwrites any existing record with the same ID.
    ///
    /// # Errors
    /// Returns `SwapError::Encryption` if the record contains sensitive material
    /// (`preimage_hex`, `refund_key_hex`, or `claim_key_hex`) but no encryption
    /// key was provided. Use `open_with_key()` or strip sensitive fields first.
    pub fn upsert(&self, record: &SwapRecord) -> Result<()> {
        if Self::contains_sensitive_material(record) && self.encryption_key.is_none() {
            return Err(SwapError::Encryption(
                "Refusing to store swap record with sensitive material \
                 (preimage_hex / refund_key_hex / claim_key_hex) without an \
                 encryption key. Call SwapStore::open_with_key() or strip \
                 sensitive fields before persisting."
                    .into(),
            ));
        }
        let mut store = self.load_all()?;
        if let Some(pos) = store.swaps.iter().position(|s| s.id == record.id) {
            store.swaps[pos] = record.clone();
        } else {
            store.swaps.push(record.clone());
        }
        self.save_all(&store)
    }

    /// Retrieve a swap record by Boltz swap ID.
    pub fn get(&self, swap_id: &str) -> Result<Option<SwapRecord>> {
        let store = self.load_all()?;
        Ok(store.swaps.into_iter().find(|s| s.id == swap_id))
    }

    /// Update the status and optional settlement txid of an existing swap.
    pub fn update_status(
        &self,
        swap_id: &str,
        new_status: SwapStatus,
        settlement_txid: Option<String>,
    ) -> Result<()> {
        let mut store = self.load_all()?;
        let now = chrono::Utc::now().timestamp();

        let swap = store
            .swaps
            .iter_mut()
            .find(|s| s.id == swap_id)
            .ok_or_else(|| SwapError::NotFound(swap_id.to_string()))?;

        swap.status = new_status;
        swap.updated_at = now;
        if settlement_txid.is_some() {
            swap.settlement_txid = settlement_txid;
        }

        self.save_all(&store)
    }

    /// List all swaps in a recoverable state (failed but funds can be reclaimed).
    pub fn list_recoverable(&self) -> Result<Vec<SwapRecord>> {
        let store = self.load_all()?;
        Ok(store
            .swaps
            .into_iter()
            .filter(|s| s.is_recoverable())
            .collect())
    }

    /// List all swaps of a given kind that are still pending.
    pub fn list_pending(&self, kind: Option<SwapKind>) -> Result<Vec<SwapRecord>> {
        let store = self.load_all()?;
        Ok(store
            .swaps
            .into_iter()
            .filter(|s| s.status.is_pending())
            .filter(|s| kind.map(|k| s.kind == k).unwrap_or(true))
            .collect())
    }

    /// List all swap records (for debugging / inspection).
    pub fn list_all(&self) -> Result<Vec<SwapRecord>> {
        Ok(self.load_all()?.swaps)
    }

    /// Remove a swap record permanently (e.g. after successful completion + cleanup).
    pub fn remove(&self, swap_id: &str) -> Result<()> {
        let mut store = self.load_all()?;
        store.swaps.retain(|s| s.id != swap_id);
        self.save_all(&store)
    }
}

// ─── Encryption helpers (AES-256-GCM) ────────────────────────────────────────

/// Encrypt plaintext bytes with AES-256-GCM.
/// Output format: [12-byte nonce][ciphertext+tag]
fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| SwapError::Encryption(format!("AES-GCM encrypt failed: {e}")))?;

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt AES-256-GCM ciphertext. Input format: [12-byte nonce][ciphertext+tag]
fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(SwapError::Encryption("Ciphertext too short".into()));
    }

    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| SwapError::Encryption("AES-GCM authentication failed".into()))
}

// ─── Platform helpers ─────────────────────────────────────────────────────────

fn satspath_dir() -> Result<PathBuf> {
    // Prefer $SATSPATH_DIR env var, then current directory's .satspath
    if let Ok(dir) = std::env::var("SATSPATH_DIR") {
        return Ok(PathBuf::from(dir));
    }
    Ok(PathBuf::from(".satspath"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SwapKind;
    use tempfile::tempdir;

    fn make_record(id: &str) -> SwapRecord {
        SwapRecord {
            id: id.to_string(),
            kind: SwapKind::Submarine,
            status: SwapStatus::Created,
            amount_sats: 10_000,
            preimage_hex: None,
            preimage_hash_hex: None,
            refund_key_hex: None,
            claim_key_hex: None,
            invoice: Some("lnbc100...".into()),
            lockup_address: Some("bc1q...".into()),
            expected_amount_sats: Some(10_100),
            timeout_block_height: Some(800_000),
            boltz_claim_pubkey: Some("02abc...".into()),
            redeem_script: None,
            lockup_txid: None,
            settlement_txid: None,
            destination_address: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        }
    }

    #[test]
    fn upsert_and_retrieve() {
        let dir = tempdir().unwrap();
        let store = SwapStore::open_plaintext_for_tests(dir.path().join("swaps.enc"));

        let rec = make_record("swap_001");
        store.upsert(&rec).unwrap();

        let found = store.get("swap_001").unwrap().unwrap();
        assert_eq!(found.id, "swap_001");
        assert_eq!(found.amount_sats, 10_000);
    }

    #[test]
    fn update_status() {
        let dir = tempdir().unwrap();
        let store = SwapStore::open_plaintext_for_tests(dir.path().join("swaps.enc"));

        store.upsert(&make_record("swap_002")).unwrap();
        store
            .update_status("swap_002", SwapStatus::InvoicePaid, Some("txid_abc".into()))
            .unwrap();

        let found = store.get("swap_002").unwrap().unwrap();
        assert_eq!(found.status, SwapStatus::InvoicePaid);
        assert_eq!(found.settlement_txid.as_deref(), Some("txid_abc"));
    }

    #[test]
    fn list_pending() {
        let dir = tempdir().unwrap();
        let store = SwapStore::open_plaintext_for_tests(dir.path().join("swaps.enc"));

        store.upsert(&make_record("swap_003")).unwrap();
        let mut done = make_record("swap_004");
        done.status = SwapStatus::InvoicePaid;
        store.upsert(&done).unwrap();

        let pending = store.list_pending(None).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "swap_003");
    }

    #[test]
    fn encryption_roundtrip() {
        let key = [0x42u8; 32];
        let plaintext = b"hello boltz swap";
        let enc = encrypt(&key, plaintext).unwrap();
        let dec = decrypt(&key, &enc).unwrap();
        assert_eq!(dec, plaintext);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key = [0x42u8; 32];
        let bad_key = [0xFFu8; 32];
        let plaintext = b"secret swap data";
        let enc = encrypt(&key, plaintext).unwrap();
        assert!(decrypt(&bad_key, &enc).is_err());
    }

    #[test]
    fn swapstore_refuses_sensitive_plaintext_records() {
        let dir = tempdir().unwrap();
        let store = SwapStore::open_plaintext(dir.path().join("swaps.enc"));

        // Non-sensitive record (no keys or preimage) must succeed.
        let clean = make_record("swap_clean");
        assert!(
            store.upsert(&clean).is_ok(),
            "clean record should store without key"
        );

        // Record with preimage_hex must be rejected without encryption key.
        let mut with_preimage = make_record("swap_preimage");
        with_preimage.preimage_hex = Some("deadbeef".repeat(8));
        let err = store.upsert(&with_preimage).unwrap_err();
        assert!(
            err.to_string().contains("sensitive material"),
            "expected sensitive-material error, got: {err}"
        );

        // Record with refund_key_hex must be rejected without encryption key.
        let mut with_refund = make_record("swap_refund");
        with_refund.refund_key_hex = Some("cafebabe".repeat(8));
        assert!(store.upsert(&with_refund).is_err());

        // Record with claim_key_hex must be rejected without encryption key.
        let mut with_claim = make_record("swap_claim");
        with_claim.claim_key_hex = Some("aabbccdd".repeat(8));
        assert!(store.upsert(&with_claim).is_err());

        // Same record with an encryption key must succeed.
        let key = [0x77u8; 32];
        let encrypted_store = SwapStore::open_encrypted_at(dir.path().join("swaps_enc.enc"), key);
        let mut sensitive = make_record("swap_sens");
        sensitive.preimage_hex = Some("deadbeef".repeat(8));
        assert!(
            encrypted_store.upsert(&sensitive).is_ok(),
            "encrypted store must accept sensitive records"
        );
    }

    #[test]
    fn contains_sensitive_material_detects_fields() {
        let clean = make_record("x");
        assert!(!SwapStore::contains_sensitive_material(&clean));

        let mut with_pre = make_record("x");
        with_pre.preimage_hex = Some("aaa".into());
        assert!(SwapStore::contains_sensitive_material(&with_pre));

        let mut with_refund = make_record("x");
        with_refund.refund_key_hex = Some("bbb".into());
        assert!(SwapStore::contains_sensitive_material(&with_refund));

        let mut with_claim = make_record("x");
        with_claim.claim_key_hex = Some("ccc".into());
        assert!(SwapStore::contains_sensitive_material(&with_claim));
    }
}
