use anyhow::Result;
use url::Url;

use super::register::cmd_register;
use satspath_core::privacy::canonical_identifier;

pub fn cmd_claim(
    claim_url_or_alias: &str,
    lightning_address: Option<&str>,
    onchain_address: Option<&str>,
) -> Result<()> {
    // If the input is a URL, extract the alias if possible, or fail if it's just a hash
    // In our invite flow, the claim_url has alias_hash, not the plaintext alias.
    // So the user must provide their alias to claim it locally.
    
    let alias = if let Ok(url) = Url::parse(claim_url_or_alias) {
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
    
    println!();
    println!("✅ Claim successful! You can now receive the pending payment.");
    Ok(())
}
