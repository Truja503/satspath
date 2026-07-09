use anyhow::Result;

use satspath_core::{create_invite, privacy::mask_identifier};

use super::open_registry;

pub async fn cmd_invite(alias: &str, amount_sats: u64, use_escrow: bool) -> Result<()> {
    let registry = open_registry()?;

    if registry.is_registered(alias) {
        println!(
            "'{}' is already registered on SatsPath. Use `satspath pay` instead.",
            mask_identifier(alias)
        );
        return Ok(());
    }

    if use_escrow {
        println!("Depositing {} sats into Custodial Escrow...", amount_sats);
        
        let client = reqwest::Client::new();
        let body = satspath_core::escrow::DepositRequest {
            receiver_alias_hash: satspath_core::privacy::identifier_hash(alias),
            amount_sats,
        };
        
        let res = client.post("http://127.0.0.1:9737/v1/escrow/deposit")
            .json(&body)
            .send()
            .await?;
            
        if !res.status().is_success() {
            anyhow::bail!("Escrow deposit failed: {}", res.text().await?);
        }
        
        let escrow_res: satspath_core::escrow::DepositResponse = res.json().await?;
        
        println!("✅ Deposit mock-funded!");
        println!("Invoice: {}", escrow_res.deposit_invoice);
        println!();
        println!("Invite link (Escrowed):");
        println!("https://satspath.local/claim?escrow_id={}&secret={}&alias={}", escrow_res.escrow_id, escrow_res.claim_secret, alias);
        println!();
        println!("Alias hash:  {}", body.receiver_alias_hash);
        println!("Amount:      {} sats", amount_sats);
    } else {
        let invite = create_invite(alias, amount_sats, None, 24 * 3600);

        println!(
            "'{}' is not registered on SatsPath.",
            mask_identifier(alias)
        );
        println!();
        println!("Invite link (Non-custodial Intent):");
        println!("{}", invite.claim_url);
        println!();
        println!("Alias hash:  {}", invite.alias_hash);
        println!("Amount:      {} sats", invite.amount_sats);
        println!("Created at:  {}", invite.created_at);
        println!();
        println!("WARNING: {}", invite.warning);
    }
    
    Ok(())
}
