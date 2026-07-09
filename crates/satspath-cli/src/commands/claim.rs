use anyhow::Result;
use url::Url;

use super::register::cmd_register;
use satspath_core::privacy::canonical_identifier;

pub async fn cmd_claim(
    claim_url_or_alias: &str,
    lightning_address: Option<&str>,
    onchain_address: Option<&str>,
) -> Result<()> {
    let mut escrow_id = None;
    let mut claim_secret = None;
    
    let alias = if let Ok(url) = Url::parse(claim_url_or_alias) {
        if let Some(e) = url.query_pairs().find(|(k, _)| k == "escrow_id") {
            escrow_id = Some(e.1.to_string());
        }
        if let Some(s) = url.query_pairs().find(|(k, _)| k == "secret") {
            claim_secret = Some(s.1.to_string());
        }
        
        if let Some(alias_param) = url.query_pairs().find(|(k, _)| k == "alias") {
            alias_param.1.to_string()
        } else {
            anyhow::bail!("Claim URL does not contain the plaintext alias. Please run `satspath claim <your-alias>` instead.");
        }
    } else {
        claim_url_or_alias.to_string()
    };

    println!("Claiming invite for alias: {}", canonical_identifier(&alias));
    
    // Claiming is essentially registering the profile for the first time, 
    // which allows the sender's router to finally resolve and pay it.
    cmd_register(&alias, lightning_address, onchain_address, None, None)?;
    
    if let (Some(eid), Some(sec)) = (escrow_id, claim_secret) {
        println!("\nExecuting Custodial Escrow Claim...");
        let client = reqwest::Client::new();
        let body = satspath_core::escrow::ClaimRequest {
            escrow_id: eid,
            claim_secret: sec,
            receiver_alias: alias,
        };
        
        let res = client.post("http://127.0.0.1:9737/v1/escrow/claim")
            .json(&body)
            .send()
            .await?;
            
        if !res.status().is_success() {
            anyhow::bail!("Escrow claim failed: {}", res.text().await?);
        }
        
        let claim_res: satspath_core::escrow::ClaimResponse = res.json().await?;
        println!("✅ {}", claim_res.message);
    } else {
        println!();
        println!("✅ Claim successful! You can now receive the pending payment.");
    }

    Ok(())
}
