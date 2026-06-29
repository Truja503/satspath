mod commands;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

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

    /// Register an alias with a signed public payment profile
    Register {
        alias: String,
        /// Wire a real Lightning Address.
        #[arg(long, alias = "ln-address")]
        lightning_address: Option<String>,
        /// Wire a real mainnet Bitcoin address.
        #[arg(long, alias = "onchain")]
        onchain_address: Option<String>,
        /// Wire an Ark server URL.
        #[arg(long)]
        ark_server: Option<String>,
        /// Wire an Ark receiver compressed secp256k1 pubkey.
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

    /// Show routing decision with live mempool fees
    Quote {
        alias: String,
        amount_sats: u64,
        /// Print machine-readable JSON. For now this is supported with --mainnet-preview.
        #[arg(long)]
        json: bool,
        /// Use mainnet public-data preview rules. No execution.
        #[arg(long)]
        mainnet_preview: bool,
        /// Fetch a real LNURL BOLT11 invoice. Requires explicit opt-in.
        #[arg(long)]
        fetch_lnurl_invoice: bool,
    },

    /// Build a mainnet-compatible public payment preview. No funds move.
    Preview {
        recipient: String,
        amount_sats: u64,
        /// Use real mainnet public data, never execution.
        #[arg(long)]
        mainnet: bool,
        /// Print only valid JSON.
        #[arg(long)]
        json: bool,
        /// Fetch a real LNURL BOLT11 invoice. Requires explicit opt-in.
        #[arg(long)]
        fetch_lnurl_invoice: bool,
    },

    /// Resolve, route, and build a public QR preview. No funds move by default.
    Pay {
        alias: String,
        amount_sats: u64,
        #[arg(long)]
        memo: Option<String>,
        #[arg(long)]
        mainnet_preview: bool,
        /// Activate experimental swap engine. Requires --testnet.
        #[arg(long)]
        experimental_swaps: bool,
        /// Target testnet instead of mainnet.
        #[arg(long)]
        testnet: bool,
        /// Print full public pointer and QR payload values.
        #[arg(long)]
        debug: bool,
    },

    /// Generate an invite for an unregistered alias
    Invite { alias: String, amount_sats: u64 },

    /// Ark direct receive/send and swap intents. Testnet-gated; mainnet execution disabled.
    Ark {
        #[command(subcommand)]
        command: ArkCommand,
    },

    /// Run the full SatsPath demo flow
    Demo,
}

#[derive(Subcommand)]
enum ArkCommand {
    /// Preview an Ark receive pointer for a registered alias.
    Receive(ArkReceiveArgs),
    /// Preview or testnet-execute a direct Ark send intent.
    Send(ArkSendArgs),
    /// Preview or testnet-execute an Ark swap intent.
    Swap(ArkSwapArgs),
}

#[derive(Args)]
struct ArkReceiveArgs {
    #[arg(long)]
    alias: String,
    #[arg(long)]
    testnet: bool,
    #[arg(long)]
    execute_testnet: bool,
}

#[derive(Args)]
struct ArkSendArgs {
    alias: String,
    amount_sats: u64,
    #[arg(long)]
    testnet: bool,
    #[arg(long)]
    execute_testnet: bool,
    #[arg(long)]
    confirm: Option<String>,
}

#[derive(Args)]
struct ArkSwapArgs {
    alias: String,
    amount_sats: u64,
    #[arg(long)]
    from: commands::ArkSwapSide,
    #[arg(long)]
    to: commands::ArkSwapSide,
    #[arg(long)]
    testnet: bool,
    #[arg(long)]
    execute_testnet: bool,
    #[arg(long)]
    confirm: Option<String>,
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
        Command::Show { alias } => commands::cmd_show(&alias).await?,
        Command::Encode {
            alias,
            amount_sats,
            memo,
        } => commands::cmd_encode(&alias, amount_sats, memo.as_deref())?,
        Command::Decode { uri } => commands::cmd_decode(&uri)?,
        Command::Quote {
            alias,
            amount_sats,
            json,
            mainnet_preview,
            fetch_lnurl_invoice,
        } => {
            commands::cmd_quote(
                &alias,
                amount_sats,
                json,
                mainnet_preview,
                fetch_lnurl_invoice,
            )
            .await?
        }
        Command::Preview {
            recipient,
            amount_sats,
            mainnet,
            json,
            fetch_lnurl_invoice,
        } => {
            commands::cmd_preview(&recipient, amount_sats, mainnet, json, fetch_lnurl_invoice)
                .await?
        }
        Command::Pay {
            alias,
            amount_sats,
            memo,
            mainnet_preview,
            experimental_swaps,
            testnet,
            debug,
        } => {
            commands::cmd_pay(
                &alias,
                amount_sats,
                memo.as_deref(),
                mainnet_preview,
                experimental_swaps,
                testnet,
                debug,
            )
            .await?
        }
        Command::Invite { alias, amount_sats } => commands::cmd_invite(&alias, amount_sats)?,
        Command::Ark { command } => match command {
            ArkCommand::Receive(args) => {
                commands::cmd_ark_receive(&args.alias, args.testnet, args.execute_testnet).await?
            }
            ArkCommand::Send(args) => {
                commands::cmd_ark_send(
                    &args.alias,
                    args.amount_sats,
                    args.testnet,
                    args.execute_testnet,
                    args.confirm.as_deref(),
                )
                .await?
            }
            ArkCommand::Swap(args) => {
                commands::cmd_ark_swap(
                    &args.alias,
                    args.amount_sats,
                    args.from,
                    args.to,
                    args.testnet,
                    args.execute_testnet,
                    args.confirm.as_deref(),
                )
                .await?
            }
        },
        Command::Demo => commands::cmd_demo().await?,
    }

    Ok(())
}
