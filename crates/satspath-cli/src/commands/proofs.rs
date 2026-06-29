//! `satspath prove` and `satspath attach-proof` — mint ownership proofs.
//!
//! Two-step, key-safe flow:
//!   1. `prove` prints the exact challenge the *method's* key holder must sign
//!      with their own wallet/Ark tooling. SatsPath needs no secret key here.
//!   2. `attach-proof` takes the resulting public signature, verifies it
//!      client-side, attaches it to the profile, and re-signs the profile with
//!      the locally stored *identity* key.
//!
//! `manual` proofs are the exception: they are self-attestations signed by the
//! identity key itself (weakest tier), so they need no external signature.

use anyhow::Result;

use satspath_core::{
    attach_signature_proof, build_manual_attestation, crypto::sign_profile, evaluate_method_trust,
    ownership_challenge_message, upsert_method_verification, PaymentMethod, ProofType,
};

use super::{keystore, open_registry, satspath_dir};

fn method_at(profile: &satspath_core::PaymentProfile, index: usize) -> Result<&PaymentMethod> {
    profile.methods.get(index).ok_or_else(|| {
        anyhow::anyhow!(
            "no method at index {index} — profile has {} method(s) (0..{})",
            profile.methods.len(),
            profile.methods.len().saturating_sub(1)
        )
    })
}

/// Print the challenge a method's key holder must sign.
pub fn cmd_prove(alias: &str, method_index: usize) -> Result<()> {
    let registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let method = method_at(&signed.profile, method_index)?;
    let descriptor = method.ownership_descriptor();
    let issued_at = chrono::Utc::now().timestamp();
    let message =
        ownership_challenge_message(&signed.profile.identity_pubkey, &descriptor, issued_at);

    let suggested_type = match method {
        PaymentMethod::Onchain { .. } => "onchain",
        PaymentMethod::Ark { .. } => "ark",
        PaymentMethod::Lightning { .. } => "manual",
    };

    println!(
        "Method [{method_index}]: {} — {descriptor}",
        method.method_name()
    );
    println!();
    println!("Sign EXACTLY this challenge with the key that controls the method");
    println!("(your wallet's signmessage for on-chain, your Ark tool for Ark):");
    println!();
    println!("----- BEGIN SATSPATH CHALLENGE -----");
    println!("{message}");
    println!("----- END SATSPATH CHALLENGE -----");
    println!();
    if matches!(method, PaymentMethod::Lightning { .. }) {
        println!("Note: Lightning methods use domain-control proofs (served at a");
        println!("well-known URL) or a `manual` self-attestation. Cryptographic");
        println!("signature proofs apply to on-chain and Ark methods.");
        println!();
    }
    println!("Then attach the resulting signature (no private key needed by SatsPath):");
    println!("  satspath attach-proof {alias} \\");
    println!(
        "    --method-index {method_index} --type {suggested_type} --issued-at {issued_at} \\"
    );
    println!("    --pubkey <compressed-pubkey-hex> --signature <der-signature-hex>");
    println!();
    println!("Or, for a self-attestation only (weakest tier):");
    println!("  satspath attach-proof {alias} --method-index {method_index} --type manual");
    Ok(())
}

/// Attach a proof to a method and re-sign the profile.
#[allow(clippy::too_many_arguments)]
pub fn cmd_attach_proof(
    alias: &str,
    method_index: usize,
    proof_type: &str,
    issued_at: Option<i64>,
    pubkey: Option<&str>,
    signature: Option<&str>,
    expires_in: Option<i64>,
) -> Result<()> {
    let mut registry = open_registry()?;
    let signed = registry
        .resolve_alias(alias)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .clone();
    let method = method_at(&signed.profile, method_index)?.clone();
    let identity_pubkey = signed.profile.identity_pubkey.clone();

    // We must re-sign the profile, so the identity key has to be available locally.
    let identity_key = keystore::load_identity_key(&satspath_dir(), &identity_pubkey)?;

    let now = chrono::Utc::now().timestamp();
    let verification = match proof_type {
        "onchain" | "ark" => {
            let proof = if proof_type == "onchain" {
                ProofType::OnchainAddressSignature
            } else {
                ProofType::ArkPubkeySignature
            };
            let issued = issued_at.ok_or_else(|| {
                anyhow::anyhow!(
                    "--issued-at is required for {proof_type} proofs (use the value \
                     printed by `satspath prove`)"
                )
            })?;
            let pk = pubkey
                .ok_or_else(|| anyhow::anyhow!("--pubkey is required for {proof_type} proofs"))?;
            let sig = signature.ok_or_else(|| {
                anyhow::anyhow!("--signature is required for {proof_type} proofs")
            })?;
            let expires_at = expires_in.map(|s| issued + s);
            attach_signature_proof(
                &method,
                &identity_pubkey,
                proof,
                pk,
                sig,
                issued,
                expires_at,
            )
            .map_err(|e| anyhow::anyhow!("proof rejected: {e}"))?
        }
        "manual" => {
            let issued = issued_at.unwrap_or(now);
            let expires_at = expires_in.map(|s| issued + s);
            build_manual_attestation(&method, &identity_pubkey, &identity_key, issued, expires_at)
                .map_err(|e| anyhow::anyhow!("{e}"))?
        }
        other => anyhow::bail!("unknown --type '{other}' (expected: onchain | ark | manual)"),
    };

    // Attach + re-sign + persist.
    let mut profile = signed.profile.clone();
    upsert_method_verification(&mut profile.method_verifications, verification);
    profile.updated_at = now;
    let resigned = sign_profile(profile, &identity_key).map_err(|e| anyhow::anyhow!("{e}"))?;
    registry
        .update_profile(resigned.clone())
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let trust = evaluate_method_trust(
        &method,
        &identity_pubkey,
        &resigned.profile.method_verifications,
        now,
        None,
    );
    println!(
        "Attached {} proof to method [{method_index}] {}.",
        proof_type,
        method.method_name()
    );
    // attach_signature_proof / build_manual_attestation verify before we persist,
    // so this badge is Verified (or NeedsNetworkCheck for domain proofs).
    println!("Ownership now: {}", trust.badge());
    println!("Profile re-signed and updated in .satspath/registry.json");
    Ok(())
}
