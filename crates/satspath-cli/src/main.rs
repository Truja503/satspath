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
    Show {
        alias: String,
        /// Fetch and re-verify domain-control proofs over the network
        #[arg(long)]
        verify_online: bool,
    },

    /// Print the ownership-proof challenge to sign for one method
    Prove {
        alias: String,
        /// Index of the method in the profile (see `satspath show`)
        #[arg(long, default_value_t = 0)]
        method_index: usize,
    },

    /// Attach an ownership proof to a method and re-sign the profile
    AttachProof {
        alias: String,
        #[arg(long, default_value_t = 0)]
        method_index: usize,
        /// Proof type: onchain | ark | domain | manual
        #[arg(long = "type")]
        proof_type: String,
        /// issued_at value printed by `satspath prove` (required for onchain/ark)
        #[arg(long)]
        issued_at: Option<i64>,
        /// Compressed secp256k1 pubkey that signed the challenge (onchain/ark)
        #[arg(long)]
        pubkey: Option<String>,
        /// DER signature (hex) over the challenge (onchain/ark)
        #[arg(long)]
        signature: Option<String>,
        /// Well-known URL to fetch+verify (domain; auto-derived for Lightning)
        #[arg(long)]
        url: Option<String>,
        /// Token the served body must contain (domain; defaults to identity pubkey)
        #[arg(long)]
        nonce: Option<String>,
        /// Verify a local copy of the served content instead of fetching (domain)
        #[arg(long)]
        body_file: Option<String>,
        /// Optional validity window in seconds from issued_at
        #[arg(long)]
        expires_in: Option<i64>,
    },

    /// Encode a universal SatsPath payment URI
    Encode {
        alias: String,
        amount_sats: u64,
        #[arg(long)]
        memo: Option<String>,
    },

    /// Decode a SatsPath payment URI
    Decode { uri: String },

    /// Show routing decision with live mempool fees + scannable QR
    Quote {
        alias: String,
        amount_sats: u64,
        /// Emit the machine-readable QuoteResponse as JSON (and nothing else)
        #[arg(long)]
        json: bool,
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

    /// Generate an invite for an unregistered alias (no funds sent, no keys generated)
    Invite { alias: String, amount_sats: u64 },

    /// Ark direct receive/send and swap intents. Testnet-gated; mainnet execution disabled.
    Ark {
        #[command(subcommand)]
        command: ArkCommand,
    },

    /// BIP-353 DNS payment-instruction resolution (mainnet preview only).
    Dns {
        #[command(subcommand)]
        command: DnsCommand,
    },

    /// Run the full SatsPath demo flow
    Demo,
}

#[derive(Subcommand)]
enum DnsCommand {
    /// Resolve ₿user@domain (or user@domain) via DNSSEC-backed BIP-353.
    Resolve(DnsResolveArgs),
}

#[derive(Args)]
struct DnsResolveArgs {
    /// The name to resolve, e.g. ₿rodrigo@satspath.dev or rodrigo@satspath.dev
    name: String,
    /// Emit the machine-readable resolution as JSON (and nothing else)
    #[arg(long)]
    json: bool,
    /// DEV ONLY: skip DNSSEC validation (never use on mainnet)
    #[arg(long)]
    allow_insecure_dns_for_dev: bool,
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
        Command::Show {
            alias,
            verify_online,
        } => commands::cmd_show(&alias, verify_online).await?,
        Command::Prove {
            alias,
            method_index,
        } => commands::cmd_prove(&alias, method_index)?,
        Command::AttachProof {
            alias,
            method_index,
            proof_type,
            issued_at,
            pubkey,
            signature,
            url,
            nonce,
            body_file,
            expires_in,
        } => {
            commands::cmd_attach_proof(
                &alias,
                method_index,
                &proof_type,
                issued_at,
                pubkey.as_deref(),
                signature.as_deref(),
                url.as_deref(),
                nonce.as_deref(),
                body_file.as_deref(),
                expires_in,
            )
            .await?
        }
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
        } => {
            if json {
                commands::cmd_quote_json(&alias, amount_sats).await?
            } else {
                commands::cmd_quote(&alias, amount_sats).await?
            }
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
        Command::Dns { command } => match command {
            DnsCommand::Resolve(args) => {
                commands::cmd_dns_resolve(&args.name, args.json, args.allow_insecure_dns_for_dev)
                    .await?
            }
        },
        Command::Demo => commands::cmd_demo().await?,
    }

    Ok(())
}
