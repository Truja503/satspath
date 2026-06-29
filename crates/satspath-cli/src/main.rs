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

    /// Register an alias with a signed payment profile
    Register {
        alias: String,
        /// Wire a real Lightning Address (e.g. trujasx@blink.sv)
        #[arg(long)]
        ln_address: Option<String>,
        /// Wire a real on-chain Bitcoin address
        #[arg(long)]
        onchain: Option<String>,
    },

    /// Show a registered profile
    Show {
        alias: String,
    },

    /// Encode a universal SatsPath payment URI
    Encode {
        alias: String,
        amount_sats: u64,
        #[arg(long)]
        memo: Option<String>,
    },

    /// Decode a SatsPath payment URI
    Decode {
        uri: String,
    },

    /// Show routing decision with live mempool fees + scannable QR
    Quote {
        alias: String,
        amount_sats: u64,
    },

    /// Resolve, route, fetch real invoice and display QR.
    /// Add --experimental-swaps --testnet to activate the swap engine (testnet only).
    Pay {
        alias: String,
        amount_sats: u64,
        #[arg(long)]
        memo: Option<String>,
        /// Activate Boltz swap engine (requires --testnet, testnet only)
        #[arg(long)]
        experimental_swaps: bool,
        /// Target testnet instead of mainnet
        #[arg(long)]
        testnet: bool,
    },

    /// Generate an invite for an unregistered alias (no funds sent, no keys generated)
    Invite {
        alias: String,
        amount_sats: u64,
    },

    /// Run the full SatsPath demo flow
    Demo,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::cmd_init()?,
        Command::Register { alias, ln_address, onchain } => {
            commands::cmd_register(&alias, ln_address.as_deref(), onchain.as_deref())?
        }
        Command::Show { alias } => commands::cmd_show(&alias)?,
        Command::Encode { alias, amount_sats, memo } => {
            commands::cmd_encode(&alias, amount_sats, memo.as_deref())?
        }
        Command::Decode { uri } => commands::cmd_decode(&uri)?,
        Command::Quote { alias, amount_sats } => {
            commands::cmd_quote(&alias, amount_sats).await?
        }
        Command::Pay { alias, amount_sats, memo, experimental_swaps, testnet } => {
            commands::cmd_pay(&alias, amount_sats, memo.as_deref(), experimental_swaps, testnet).await?
        }
        Command::Invite { alias, amount_sats } => {
            commands::cmd_invite(&alias, amount_sats)?
        }
        Command::Demo => commands::cmd_demo().await?,
    }

    Ok(())
}
