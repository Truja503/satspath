use anyhow::Result;

use super::{
    cmd_decode, cmd_init, cmd_invite, cmd_pay, cmd_quote, cmd_register, cmd_show,
};

pub async fn cmd_demo() -> Result<()> {
    println!("══════════════════════════════════════════════════");
    println!("  SatsPath — Full Demo Flow");
    println!("══════════════════════════════════════════════════");
    println!();

    step(1, "Initialize SatsPath");
    cmd_init()?;
    println!();

    step(2, "Register rodrigo@satspath.dev");
    // Re-registration is idempotent for demo — if already registered, show message.
    cmd_register("rodrigo@satspath.dev", None, None)?;
    println!();

    step(3, "Show signed profile");
    cmd_show("rodrigo@satspath.dev").await?;
    println!();

    step(4, "Encode universal payment request (21,000 sats, memo: coffee)");
    let uri = satspath_core::codec::encode_payment_request(
        "rodrigo@satspath.dev",
        Some(21_000),
        Some("coffee"),
    )?;
    println!("URI: {}", uri);
    println!();

    step(5, "Decode payment request");
    cmd_decode(&uri)?;
    println!();

    step(6, "Get route quote for 21,000 sats");
    cmd_quote("rodrigo@satspath.dev", 21_000).await?;
    println!();

    step(7, "Simulate payment of 21,000 sats");

  cmd_pay("rodrigo@satspath.dev", 21_000, None, false, false).await?;
    println!();

    step(8, "Try paying an unknown user (julian@example.com)");
    println!("Attempting to pay julian@example.com...");
    let registry = super::open_registry()?;
    if registry.is_registered("julian@example.com") {
        println!("julian@example.com is already registered.");
    } else {
        println!("julian@example.com is not registered. Generating invite...");
        println!();

        step(9, "Generate invite link for julian@example.com");
        cmd_invite("julian@example.com", 21_000)?;
    }

    println!();
    println!("══════════════════════════════════════════════════");
    println!("  Demo complete.");
    println!("══════════════════════════════════════════════════");
    println!();
    println!("Next steps:");
    println!("  satspath register <your-alias>");
    println!("  satspath pay <alias> <amount_sats>");
    println!("  satspath invite <unregistered@example.com> <amount>");
    Ok(())
}

fn step(n: u32, title: &str) {
    println!("──────────────────────────────────────────────────");
    println!("Step {}: {}", n, title);
    println!("──────────────────────────────────────────────────");
}
