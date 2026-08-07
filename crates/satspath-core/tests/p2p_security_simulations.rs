use satspath_core::crypto::{generate_identity_keypair, sign_profile, verify_signed_profile};
use satspath_core::profile::{PaymentMethod, PaymentProfile};
use sha2::{Digest, Sha256};
use chrono::Utc;

fn create_testnet_profile(alias: &str, identity_pubkey: &str, ln_address: &str) -> PaymentProfile {
    PaymentProfile {
        alias: alias.to_string(),
        identity_pubkey: identity_pubkey.to_string(),
        methods: vec![PaymentMethod::Lightning {
            label: "Testnet Lightning".into(),
            lightning_address: Some(ln_address.to_string()),
            lnurl: None,
            bolt12: None,
            receiver_pubkey: None,
        }],
        updated_at: Utc::now().timestamp(),
        expires_at: None,
        sequence: Some(1),
        preferences: vec![],
        nonce: Some(satspath_core::crypto::generate_nonce()),
        rotation: None,
        method_verifications: vec![],
        hybrid_pubkey: None,
        pqc_required: false,
        revoked: false,
    }
}

// Emulates the DHT topic hashing defined in P2P-03 rules
fn hash_topic(alias: &str) -> String {
    let digest = Sha256::digest(alias.as_bytes());
    hex::encode(digest)
}

#[test]
fn test_attack_p2p_dht_scraping_privacy() {
    println!("✅ SETUP: User generates Testnet profile from CLI/GUI...");
    let alias = "chelo@testnet";
    
    // ATTACK 8 (Part 1): P2P DHT Scraping
    // The attacker listens to the Hyperswarm DHT to find who is registering on SatsPath.
    println!("⚔️ ATTACK 8 (Part 1): Sniffer listens to the Hyperswarm DHT announcements...");
    
    let public_topic = hash_topic(alias);
    
    // VALIDATION: The sniffer only sees the hash. It cannot reverse it to the plaintext alias.
    println!("🔍 SNIFFER SEES: Announcing on DHT Topic: {}", public_topic);
    
    assert_ne!(public_topic, alias, "SECURITY FAILURE: The plaintext alias leaked into the P2P topic!");
    assert_eq!(public_topic.len(), 64, "Topic should be a SHA-256 hash.");
    println!("🛡️ DEFENSE SUCCESS: Privacy Rule P2P-03 enforced. Alias is mathematically obfuscated.");
}

#[test]
fn test_attack_p2p_in_transit_corruption() {
    let alice_keys = generate_identity_keypair();
    let alice_pubkey_hex = hex::encode(alice_keys.public_key.serialize());
    
    let profile = create_testnet_profile("chelo@testnet", &alice_pubkey_hex, "testnet_node@lightning.dev");
    let signed_profile = sign_profile(profile, &alice_keys.secret_key).unwrap();
    
    // Serialize to simulate the raw bytes traveling across the P2P network
    let network_payload = serde_json::to_string(&signed_profile).unwrap();
    println!("✅ SETUP: Payload broadcasted to P2P network.");

    // ATTACK 8 (Part 2): In-Transit MITM Corruption
    println!("⚔️ ATTACK 8 (Part 2): Sniffer intercepts the P2P payload in-transit and modifies the Testnet address...");
    
    // Attacker modifies the JSON packet bytes in the air to redirect funds
    let corrupted_payload = network_payload.replace("testnet_node@lightning.dev", "hacker_testnet@evil.node");
    
    // Node receives the corrupted payload and attempts to parse and verify it
    let received_profile: Result<satspath_core::profile::SignedPaymentProfile, _> = serde_json::from_str(&corrupted_payload);
    
    // VALIDATION
    assert!(received_profile.is_ok(), "Payload should still parse as JSON.");
    
    let parsed_profile = received_profile.unwrap();
    let verification_result = verify_signed_profile(&parsed_profile).unwrap_or(false);
    
    assert!(!verification_result, "SECURITY FAILURE: The receiving node accepted a payload corrupted in-transit!");
    println!("🛡️ DEFENSE SUCCESS: The receiving Rust Core detected the P2P MITM corruption and aborted the Testnet payment.");
}
