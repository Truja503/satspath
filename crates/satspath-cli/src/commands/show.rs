use anyhow::Result;

use satspath_core::{
    crypto::{fingerprint_pubkey, verify_signed_profile},
    evaluate_method_trust,
    privacy::{mask_address, mask_identifier, mask_invoice, mask_pubkey},
    MethodTrust, PaymentMethod,
};

use super::get_resolver;
use satspath_core::resolver::ProfileResolver;

pub async fn cmd_show(alias: &str) -> Result<()> {
    let resolver = get_resolver()?;
    let signed = resolver
        .resolve_alias(alias)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let valid = verify_signed_profile(&signed)?;
    let fp = fingerprint_pubkey(&signed.profile.identity_pubkey)?;
    let now = chrono::Utc::now().timestamp();

    println!("Alias:          {}", mask_identifier(&signed.profile.alias));
    println!(
        "Identity pubkey:{}",
        mask_pubkey(&signed.profile.identity_pubkey)
    );
    println!("Fingerprint:    {}", fp);
    println!(
        "Signature valid: {}",
        if valid {
            "yes"
        } else {
            "NO — profile may be tampered!"
        }
    );
    println!("Updated at:     {}", signed.profile.updated_at);

    // Per-method ownership trust, re-verified client-side (no network here, so
    // domain-control proofs are reported as "needs fetch" rather than trusted).
    let trusts: Vec<MethodTrust> = signed
        .profile
        .methods
        .iter()
        .map(|m| {
            evaluate_method_trust(
                m,
                &signed.profile.identity_pubkey,
                &signed.profile.method_verifications,
                now,
                None,
            )
        })
        .collect();

    let verified = trusts.iter().filter(|t| t.is_verified()).count();
    let self_asserted = trusts.iter().filter(|t| t.is_self_asserted()).count();
    let suspicious = trusts.iter().filter(|t| t.is_suspicious()).count();
    println!(
        "Ownership:      {} of {} method(s) independently verified",
        verified,
        trusts.len()
    );
    if self_asserted > 0 {
        println!(
            "  {} method(s) self-asserted only (no independent proof).",
            self_asserted
        );
    }
    if suspicious > 0 {
        println!(
            "  ⚠  {} method(s) carry an INVALID or EXPIRED proof — do not trust them.",
            suspicious
        );
    }

    println!();
    println!("Methods:");
    for (method, trust) in signed.profile.methods.iter().zip(&trusts) {
        print_method(method, trust);
    }
    Ok(())
}

fn print_method(method: &PaymentMethod, trust: &MethodTrust) {
    match method {
        PaymentMethod::Lightning {
            label,
            lightning_address,
            lnurl,
            bolt12,
            receiver_pubkey,
        } => {
            println!("  - {} [Lightning]   {}", label, trust.badge());
            if let Some(la) = lightning_address {
                println!("      Lightning Address: {}", mask_identifier(la));
            }
            if let Some(url) = lnurl {
                println!("      LNURL: {}", mask_address(url));
            }
            if let Some(b12) = bolt12 {
                println!("      BOLT12: {}", mask_invoice(b12));
            }
            if let Some(pubkey) = receiver_pubkey {
                println!("      Receiver pubkey: {}", mask_pubkey(pubkey));
            }
        }
        PaymentMethod::Onchain {
            label,
            network,
            address,
            pubkey_hint,
            descriptor_hint,
        } => {
            println!("  - {} [On-chain]   {}", label, trust.badge());
            println!("      Network: {:?}", network);
            println!("      Address: {}", mask_address(address));
            if let Some(hint) = pubkey_hint {
                println!("      Pubkey hint: {}", mask_pubkey(hint));
            }
            PaymentMethod::Ark {
                label,
                server,
                pubkey,
                vtxo_pointer,
                proof,
                expires_at,
            } => {
                println!("  - {} [Ark]", label);
                println!("      Server: {}", mask_address(server));
                println!("      Pubkey: {}", mask_pubkey(pubkey));
                if vtxo_pointer.is_some() {
                    println!("      VTXO pointer: present");
                }
                println!(
                    "      Ownership proof: {}",
                    if proof.is_some() {
                        "claimed"
                    } else {
                        "not provided"
                    }
                );
                if let Some(expires_at) = expires_at {
                    println!("      Expires at: {}", expires_at);
                }
            }
        }
    }
    // Surface the failure reason so a suspicious method is actionable.
    if let MethodTrust::Invalid(reason) = trust {
        println!("      ⚠  proof rejected: {}", reason);
    }
}
