mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "satspath",
    about = "SatsPath — universal Bitcoin payment resolver and router",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize SatsPath local state (.satspath/ directory)
    Init,

    /// Register an alias and create a signed payment profile
    Register {
        alias: String,
        #[arg(long)]
        lightning_address: Option<String>,
        #[arg(long)]
        onchain_address: Option<String>,
        #[arg(long)]
        ark_server: Option<String>,
        #[arg(long)]
        ark_pubkey: Option<String>,
    },

    /// Show a registered profile
    Show { alias: String },

    /// Encode a universal SatsPath payment URI
    Encode {
        alias: String,
        amount_sats: u64,
        #[arg(long)]
        memo: Option<String>,
    },

    /// Decode a SatsPath payment URI
    Decode { uri: String },

    /// Get a route quote for an alias and amount
    Quote { alias: String, amount_sats: u64 },

    /// Simulate a payment to an alias
    Pay {
        alias: String,
        amount_sats: u64,
        #[arg(long)]
        mainnet_preview: bool,
        #[arg(long)]
        experimental_swaps: bool,
        #[arg(long)]
        testnet: bool,
        #[arg(long)]
        debug: bool,
    },

    /// Generate an invite for an unregistered alias
    Invite { alias: String, amount_sats: u64 },

    /// Run the full SatsPath demo flow
    Demo,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::cmd_init()?,
        Command::Register {
            alias,
            lightning_address,
            onchain_address,
            ark_server,
            ark_pubkey,
        } => commands::cmd_register(
            &alias,
            lightning_address.as_deref(),
            onchain_address.as_deref(),
            ark_server.as_deref(),
            ark_pubkey.as_deref(),
        )?,
        Command::Show { alias } => commands::cmd_show(&alias)?,
        Command::Encode {
            alias,
            amount_sats,
            memo,
        } => commands::cmd_encode(&alias, amount_sats, memo.as_deref())?,
        Command::Decode { uri } => commands::cmd_decode(&uri)?,
        Command::Quote { alias, amount_sats } => commands::cmd_quote(&alias, amount_sats).await?,
        Command::Pay {
            alias,
            amount_sats,
            mainnet_preview,
            experimental_swaps,
            testnet,
            debug,
        } => {
            commands::cmd_pay(
                &alias,
                amount_sats,
                mainnet_preview,
                experimental_swaps,
                testnet,
                debug,
            )
            .await?
        }
        Command::Invite { alias, amount_sats } => commands::cmd_invite(&alias, amount_sats)?,
        Command::Demo => commands::cmd_demo().await?,
    }

    Ok(())
}
