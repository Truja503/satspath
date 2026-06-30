//! `satspathd` is a local receiver-profile daemon.
//!
//! It manages SatsPath profile identity and public receive pointers only. It
//! does not move funds, sign Bitcoin transactions, broadcast transactions, or
//! store Bitcoin wallet seeds/spending keys.

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::Parser;
use qrcode::{Color, QrCode};
use satspath_core::ark::validate_ark_server_url;
use satspath_core::bip321::{parse_bip321, ParsedBip321Uri};
use satspath_core::bip353::{resolve_bip353_with, Bip353Resolution, DnssecPolicy, DohTxtResolver};
use satspath_core::crypto::{
    fingerprint_pubkey, generate_identity_keypair, sign_profile, verify_signed_profile,
};
use satspath_core::peer_registry::{LocalPeerRegistry, PeerRecord, PeerRegistryBackend};
use satspath_core::privacy::mask_identifier;
use satspath_core::registry::Registry;
use satspath_core::resolver::ChainResolver;
use satspath_core::resolvers::{bip353::Bip353Resolver, http::HttpResolver, nostr::NostrResolver};
use satspath_core::validation::{
    assert_no_private_material, validate_amount_sats, validate_bitcoin_address,
    validate_compressed_pubkey, validate_lightning_address,
};
use satspath_core::{BitcoinNetwork, PaymentMethod, PaymentProfile, SignedPaymentProfile};
use satspath_router::QuoteResponse;
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const DEFAULT_BIND: &str = "127.0.0.1:9737";
const DEFAULT_NETWORK: &str = "devnet";
const WALLET_FILE: &str = "wallet.json";
const IDENTITY_SUBDIR: &str = "identity";

#[derive(Parser)]
#[command(
    name = "satspathd",
    about = "Local SatsPath receiver-profile daemon",
    version = "0.1.0"
)]
struct Cli {
    /// HTTP bind address. Defaults to SATSPATHD_BIND or 127.0.0.1:9737.
    #[arg(long)]
    bind: Option<String>,
    /// SatsPath network label. Defaults to SATSPATH_NETWORK or devnet.
    #[arg(long)]
    network: Option<String>,
    /// SatsPath home directory. Defaults to SATSPATH_HOME or ~/.satspath.
    #[arg(long)]
    home: Option<PathBuf>,
    /// Start the optional Holepunch P2P profile publisher bridge.
    #[arg(long)]
    p2p: bool,
    /// Do not open the wallet UI in a browser on startup.
    #[arg(long)]
    no_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WalletState {
    #[serde(skip_serializing_if = "Option::is_none")]
    alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity_pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lightning_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    onchain_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ark_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ark_pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<i64>,
}

#[derive(Clone)]
struct AppState {
    home: PathBuf,
    bind: SocketAddr,
    network: String,
    open_ui: bool,
    p2p: Arc<Mutex<P2pBridge>>,
}

struct P2pBridge {
    enabled: bool,
    status: String,
    child: Option<Child>,
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    daemon: &'static str,
    version: &'static str,
    bind: String,
    network: String,
    home: String,
    wallet_initialized: bool,
    alias: Option<String>,
    identity_fingerprint: Option<String>,
    methods: Vec<String>,
    p2p: P2pStatus,
    safety: SafetyStatus,
}

#[derive(Debug, Serialize)]
struct NodeResponse {
    status: StatusResponse,
    profile: ProfileResponse,
    peers: PeersResponse,
    connections: ConnectionsResponse,
}

#[derive(Debug, Serialize)]
struct PeersResponse {
    active_count: usize,
    peers: Vec<PeerView>,
}

#[derive(Debug, Serialize)]
struct PeerView {
    identifier_hash: String,
    display_hint: String,
    identity_fingerprint: Option<String>,
    updated_at: i64,
    expires_at: Option<i64>,
    active: bool,
    methods: Vec<String>,
    record: PeerRecord,
}

#[derive(Debug, Serialize)]
struct ConnectionsResponse {
    active_count: usize,
    connections: Vec<ConnectionView>,
}

#[derive(Debug, Serialize)]
struct ConnectionView {
    kind: &'static str,
    status: String,
    active: bool,
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct P2pStatus {
    enabled: bool,
    status: String,
    active: bool,
    pid: Option<u32>,
}

#[derive(Debug, Serialize)]
struct SafetyStatus {
    moves_funds: bool,
    signs_bitcoin_transactions: bool,
    broadcasts_transactions: bool,
    stores_wallet_seeds_or_spending_keys: bool,
    manages_signed_profiles: bool,
}

#[derive(Debug, Deserialize)]
struct ProfileUpdateRequest {
    alias: Option<String>,
    lightning_address: Option<String>,
    onchain_address: Option<String>,
    ark_server: Option<String>,
    ark_pubkey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AliasRequest {
    alias: String,
}

#[derive(Debug, Deserialize)]
struct QuoteRequest {
    recipient: String,
    amount_sats: u64,
}

#[derive(Debug, Deserialize)]
struct PayRequest {
    recipient: String,
    amount_sats: u64,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DnsResolveRequest {
    name: String,
    #[serde(default)]
    allow_insecure_dns_for_dev: bool,
}

#[derive(Debug, Serialize)]
struct ProfileResponse {
    wallet: WalletState,
    signed_profile: Option<SignedPaymentProfile>,
    signature_valid: Option<bool>,
}

#[derive(Debug, Serialize)]
struct PreviewResponse<T: Serialize> {
    mode: &'static str,
    warnings: Vec<&'static str>,
    quote: T,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum PayResponse {
    WalletHandoff {
        decision_protocol: &'static str,
        recipient: String,
        amount_sats: u64,
        quote: QuoteResponse,
        payment_payload: String,
        qr_svg: String,
        handoff: WalletHandoff,
        safety: SafetyStatus,
    },
    InviteCreated {
        decision_protocol: &'static str,
        recipient_hint: String,
        amount_sats: u64,
        quote: QuoteResponse,
        safety: SafetyStatus,
    },
    NoRoute {
        decision_protocol: &'static str,
        reason: String,
        quote: QuoteResponse,
        safety: SafetyStatus,
    },
    InvalidSignature {
        decision_protocol: &'static str,
        quote: QuoteResponse,
        safety: SafetyStatus,
    },
}

#[derive(Debug, Serialize)]
struct WalletHandoff {
    mode: &'static str,
    instruction: &'static str,
    opens_external_wallet: bool,
    daemon_executes_payment: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
enum DnsResolveResponse {
    Ok {
        resolution: Bip353Resolution,
        parsed: ParsedBip321Uri,
    },
    Error {
        name: String,
        error: String,
        strict_mode: bool,
    },
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let bind = cli
        .bind
        .or_else(|| std::env::var("SATSPATHD_BIND").ok())
        .unwrap_or_else(|| DEFAULT_BIND.to_string())
        .parse::<SocketAddr>()
        .context("invalid bind address")?;
    let network = cli
        .network
        .or_else(|| std::env::var("SATSPATH_NETWORK").ok())
        .unwrap_or_else(|| DEFAULT_NETWORK.to_string());
    let home = cli
        .home
        .or_else(|| std::env::var_os("SATSPATH_HOME").map(PathBuf::from))
        .unwrap_or_else(default_home);
    let p2p_requested = cli.p2p || env_truthy("SATSPATHD_P2P");

    fs::create_dir_all(&home).context("creating SATSPATH_HOME")?;
    let wallet = load_or_create_identity(&home)?;
    let mut bridge = P2pBridge {
        enabled: p2p_requested,
        status: "disabled".into(),
        child: None,
    };
    if p2p_requested {
        match start_p2p_bridge(&home, &wallet) {
            Ok((status, child)) => {
                bridge.status = status;
                bridge.child = Some(child);
            }
            Err(e) => {
                bridge.status = format!("inactive: {e}");
            }
        }
    }

    let state = AppState {
        home,
        bind,
        network,
        open_ui: !cli.no_open,
        p2p: Arc::new(Mutex::new(bridge)),
    };

    print_startup_status(&state)?;
    serve(state).await
}

async fn serve(state: AppState) -> Result<()> {
    let server = Server::http(state.bind).map_err(|e| anyhow::anyhow!("{e}"))?;
    let url = format!("http://{}/", state.bind);
    println!("Wallet UI → {url}");
    if state.open_ui {
        open_browser(&url);
    }
    let state = Arc::new(state);
    for request in server.incoming_requests() {
        let state = Arc::clone(&state);
        if let Err(e) = handle_request(request, &state).await {
            eprintln!("request error: {e}");
        }
    }
    Ok(())
}

async fn handle_request(mut request: Request, state: &AppState) -> Result<()> {
    let method = request.method().clone();
    let path = request.url().split('?').next().unwrap_or("/").to_string();
    let response = match (method, path.as_str()) {
        (Method::Options, _) => empty_response(StatusCode(204)),
        (Method::Get, "/") => html_response(INDEX_HTML),
        (Method::Get, "/v1/receive") => json_result(StatusCode(200), receive_view(state)),
        (Method::Get, "/health") => {
            json_response(StatusCode(200), &serde_json::json!({"ok": true}))
        }
        (Method::Get, "/v1/node") => json_result(StatusCode(200), node_response(state)),
        (Method::Get, "/v1/status") => json_result(StatusCode(200), status_response(state)),
        (Method::Get, "/v1/profile") => json_result(StatusCode(200), profile_response(state)),
        (Method::Get, "/v1/peers") => json_result(StatusCode(200), peers_response(state)),
        (Method::Get, "/v1/connections") => {
            json_result(StatusCode(200), connections_response(state))
        }
        (Method::Put, "/v1/profile") | (Method::Post, "/v1/profile") => {
            match read_json::<ProfileUpdateRequest>(&mut request)
                .and_then(|body| update_profile(state, body))
            {
                Ok(resp) => json_response(StatusCode(200), &resp),
                Err(e) => json_error(StatusCode(400), e),
            }
        }
        (Method::Post, "/v1/profile/methods") => {
            match read_json::<ProfileUpdateRequest>(&mut request)
                .and_then(|body| update_profile_methods(state, body))
            {
                Ok(resp) => json_response(StatusCode(200), &resp),
                Err(e) => json_error(StatusCode(400), e),
            }
        }
        (Method::Post, "/v1/resolve") => match read_json::<AliasRequest>(&mut request)
            .and_then(|body| resolve_profile(state, &body.alias))
        {
            Ok(profile) => json_response(StatusCode(200), &profile),
            Err(e) => json_error(StatusCode(404), e),
        },
        (Method::Post, "/v1/quote") => match read_json::<QuoteRequest>(&mut request) {
            Ok(body) => json_response(StatusCode(200), &quote_response(state, body).await),
            Err(e) => json_error(StatusCode(400), e),
        },
        (Method::Post, "/v1/pay") => match read_json::<PayRequest>(&mut request) {
            Ok(body) => json_response(StatusCode(200), &pay_response(state, body).await),
            Err(e) => json_error(StatusCode(400), e),
        },
        (Method::Post, "/v1/dns/resolve") => match read_json::<DnsResolveRequest>(&mut request) {
            Ok(body) => json_response(StatusCode(200), &dns_resolve_response(body).await),
            Err(e) => json_error(StatusCode(400), e),
        },
        (Method::Post, "/v1/preview") => match read_json::<QuoteRequest>(&mut request) {
            Ok(body) => {
                let quote = quote_response(state, body).await;
                json_response(
                    StatusCode(200),
                    &PreviewResponse {
                        mode: "preview_only",
                        warnings: safety_warnings(),
                        quote,
                    },
                )
            }
            Err(e) => json_error(StatusCode(400), e),
        },
        _ => json_error(StatusCode(404), anyhow::anyhow!("endpoint not found")),
    };
    request.respond(response)?;
    Ok(())
}

fn update_profile(state: &AppState, body: ProfileUpdateRequest) -> Result<ProfileResponse> {
    let mut wallet = load_or_create_identity(&state.home)?;
    if let Some(alias) = &body.alias {
        assert_no_private_material(alias)?;
        wallet.alias = Some(alias.clone());
    }
    apply_method_updates(&mut wallet, &state.network, body, true)?;
    sign_and_store(&state.home, &mut wallet, &state.network)?;
    save_wallet(&state.home, &wallet)?;
    profile_response(state)
}

fn update_profile_methods(state: &AppState, body: ProfileUpdateRequest) -> Result<ProfileResponse> {
    let mut wallet = load_or_create_identity(&state.home)?;
    if wallet.alias.is_none() {
        anyhow::bail!("set alias first with PUT /v1/profile");
    }
    apply_method_updates(&mut wallet, &state.network, body, false)?;
    sign_and_store(&state.home, &mut wallet, &state.network)?;
    save_wallet(&state.home, &wallet)?;
    profile_response(state)
}

fn apply_method_updates(
    wallet: &mut WalletState,
    network: &str,
    body: ProfileUpdateRequest,
    allow_empty: bool,
) -> Result<()> {
    let has_method = body.lightning_address.is_some()
        || body.onchain_address.is_some()
        || body.ark_server.is_some()
        || body.ark_pubkey.is_some();
    if !allow_empty && !has_method {
        anyhow::bail!("provide at least one receive method");
    }

    if let Some(addr) = body.lightning_address {
        validate_lightning_address(&addr)?;
        wallet.lightning_address = Some(addr);
    }
    if let Some(addr) = body.onchain_address {
        validate_bitcoin_address(&addr, bitcoin_network(network))?;
        wallet.onchain_address = Some(addr);
    }
    match (body.ark_server, body.ark_pubkey) {
        (Some(server), Some(pubkey)) => {
            validate_ark_server_url(&server)?;
            validate_compressed_pubkey(&pubkey)?;
            wallet.ark_server = Some(server);
            wallet.ark_pubkey = Some(pubkey);
        }
        (None, None) => {}
        _ => anyhow::bail!("ark_server and ark_pubkey must be provided together"),
    }
    wallet.updated_at = Some(now());
    Ok(())
}

fn sign_and_store(home: &Path, wallet: &mut WalletState, network: &str) -> Result<()> {
    let alias = wallet
        .alias
        .clone()
        .ok_or_else(|| anyhow::anyhow!("profile alias is required"))?;
    let identity_pubkey = wallet
        .identity_pubkey
        .clone()
        .ok_or_else(|| anyhow::anyhow!("identity is not initialized"))?;
    let methods = build_methods(wallet, network);
    if methods.is_empty() {
        anyhow::bail!("profile needs at least one public receive method");
    }

    let secret = load_identity_key(home, &identity_pubkey)?;
    let profile = PaymentProfile {
        alias,
        identity_pubkey,
        methods,
        updated_at: now(),
        expires_at: None,
        method_verifications: Vec::new(),
    };
    let signed = sign_profile(profile, &secret)?;
    Registry::open(home)?.update_profile(signed)?;
    Ok(())
}

fn resolve_profile(state: &AppState, alias: &str) -> Result<SignedPaymentProfile> {
    let signed = Registry::open(&state.home)?.resolve_alias(alias)?.clone();
    if !verify_signed_profile(&signed)? {
        anyhow::bail!("stored profile signature is invalid");
    }
    Ok(signed)
}

async fn quote_response(state: &AppState, body: QuoteRequest) -> satspath_router::QuoteResponse {
    if let Err(e) = validate_amount_sats(body.amount_sats) {
        return satspath_router::QuoteResponse::NoRoute {
            reason: e.to_string(),
        };
    }
    let resolver = resolver_chain(&state.home);
    satspath_router::quote_with_resolver(&resolver, &body.recipient, body.amount_sats).await
}

async fn pay_response(state: &AppState, body: PayRequest) -> PayResponse {
    if let Err(e) = validate_amount_sats(body.amount_sats) {
        let quote = QuoteResponse::NoRoute {
            reason: e.to_string(),
        };
        return PayResponse::NoRoute {
            decision_protocol: "satspathd.v1",
            reason: e.to_string(),
            quote,
            safety: safety_status(),
        };
    }
    if let Some(memo) = &body.memo {
        if let Err(e) = assert_no_private_material(memo) {
            let quote = QuoteResponse::NoRoute {
                reason: e.to_string(),
            };
            return PayResponse::NoRoute {
                decision_protocol: "satspathd.v1",
                reason: e.to_string(),
                quote,
                safety: safety_status(),
            };
        }
    }

    let quote = quote_response(
        state,
        QuoteRequest {
            recipient: body.recipient.clone(),
            amount_sats: body.amount_sats,
        },
    )
    .await;

    match quote.clone() {
        QuoteResponse::Ok { qr, .. } => match qr_svg(&qr) {
            Ok(qr_svg) => PayResponse::WalletHandoff {
                decision_protocol: "satspathd.v1",
                recipient: body.recipient,
                amount_sats: body.amount_sats,
                quote,
                payment_payload: qr,
                qr_svg,
                handoff: WalletHandoff {
                    mode: "external_wallet",
                    instruction: "Open or scan payment_payload with a wallet you control.",
                    opens_external_wallet: true,
                    daemon_executes_payment: false,
                },
                safety: safety_status(),
            },
            Err(e) => PayResponse::NoRoute {
                decision_protocol: "satspathd.v1",
                reason: e.to_string(),
                quote,
                safety: safety_status(),
            },
        },
        QuoteResponse::NotRegistered { .. } => PayResponse::InviteCreated {
            decision_protocol: "satspathd.v1",
            recipient_hint: mask_identifier(&body.recipient),
            amount_sats: body.amount_sats,
            quote,
            safety: safety_status(),
        },
        QuoteResponse::NoRoute { reason } => PayResponse::NoRoute {
            decision_protocol: "satspathd.v1",
            reason,
            quote,
            safety: safety_status(),
        },
        QuoteResponse::InvalidSignature { .. } => PayResponse::InvalidSignature {
            decision_protocol: "satspathd.v1",
            quote,
            safety: safety_status(),
        },
    }
}

async fn dns_resolve_response(body: DnsResolveRequest) -> DnsResolveResponse {
    let policy = if body.allow_insecure_dns_for_dev {
        DnssecPolicy::DevInsecure
    } else {
        DnssecPolicy::Strict
    };
    let resolver = DohTxtResolver::new();
    match resolve_bip353_with(&resolver, &body.name, policy, now()).await {
        Ok(resolution) => match parse_bip321(&resolution.bitcoin_uri) {
            Ok(parsed) => DnsResolveResponse::Ok { resolution, parsed },
            Err(e) => DnsResolveResponse::Error {
                name: body.name,
                error: e.to_string(),
                strict_mode: policy == DnssecPolicy::Strict,
            },
        },
        Err(e) => DnsResolveResponse::Error {
            name: body.name,
            error: e.to_string(),
            strict_mode: policy == DnssecPolicy::Strict,
        },
    }
}

fn profile_response(state: &AppState) -> Result<ProfileResponse> {
    let wallet = load_wallet(&state.home)?;
    let signed_profile = match wallet.alias.as_deref() {
        Some(alias) => Registry::open(&state.home)
            .and_then(|registry| registry.resolve_alias(alias).cloned())
            .ok(),
        None => None,
    };
    let signature_valid = signed_profile
        .as_ref()
        .map(verify_signed_profile)
        .transpose()?;
    Ok(ProfileResponse {
        wallet,
        signed_profile,
        signature_valid,
    })
}

fn node_response(state: &AppState) -> Result<NodeResponse> {
    Ok(NodeResponse {
        status: status_response(state)?,
        profile: profile_response(state)?,
        peers: peers_response(state)?,
        connections: connections_response(state)?,
    })
}

fn peers_response(state: &AppState) -> Result<PeersResponse> {
    let registry = LocalPeerRegistry::open(&state.home)?;
    let mut peers = Vec::new();
    for hash in registry.list_hashes()? {
        if let Some(record) = registry.get_hash(&hash) {
            peers.push(peer_view(record));
        }
    }
    peers.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
    let active_count = peers.iter().filter(|peer| peer.active).count();
    Ok(PeersResponse {
        active_count,
        peers,
    })
}

fn peer_view(record: PeerRecord) -> PeerView {
    let active = record
        .expires_at
        .map(|expires_at| expires_at > now())
        .unwrap_or(true);
    let identity_fingerprint = fingerprint_pubkey(&record.identity_pubkey).ok();
    let mut methods = Vec::new();
    if record.pointers.lightning.is_some() {
        methods.push("Lightning".into());
    }
    if record.pointers.onchain.is_some() {
        methods.push("Onchain".into());
    }
    if record.pointers.ark.is_some() {
        methods.push("Ark".into());
    }
    PeerView {
        identifier_hash: record.identifier_hash.clone(),
        display_hint: record.display_hint.clone(),
        identity_fingerprint,
        updated_at: record.updated_at,
        expires_at: record.expires_at,
        active,
        methods,
        record,
    }
}

fn connections_response(state: &AppState) -> Result<ConnectionsResponse> {
    let peers = peers_response(state)?;
    let p2p = p2p_status(state);
    let mut connections = Vec::new();
    connections.push(ConnectionView {
        kind: "p2p_bridge",
        status: p2p.status.clone(),
        active: p2p.active,
        detail: p2p.pid.map(|pid| format!("pid:{pid}")),
    });
    connections.push(ConnectionView {
        kind: "peer_registry",
        status: format!("{} active peer(s)", peers.active_count),
        active: peers.active_count > 0,
        detail: Some(
            state
                .home
                .join("peers/registry.local.json")
                .display()
                .to_string(),
        ),
    });
    let active_count = connections
        .iter()
        .filter(|connection| connection.active)
        .count();
    Ok(ConnectionsResponse {
        active_count,
        connections,
    })
}

fn status_response(state: &AppState) -> Result<StatusResponse> {
    let wallet = load_wallet(&state.home)?;
    let methods = build_methods(&wallet, &state.network)
        .iter()
        .map(|method| method.method_name().to_string())
        .collect();
    let identity_fingerprint = wallet
        .identity_pubkey
        .as_deref()
        .map(fingerprint_pubkey)
        .transpose()?;
    let p2p = p2p_status(state);
    Ok(StatusResponse {
        daemon: "satspathd",
        version: env!("CARGO_PKG_VERSION"),
        bind: state.bind.to_string(),
        network: state.network.clone(),
        home: state.home.display().to_string(),
        wallet_initialized: wallet.identity_pubkey.is_some(),
        alias: wallet.alias,
        identity_fingerprint,
        methods,
        p2p: P2pStatus {
            enabled: p2p.enabled,
            status: p2p.status,
            active: p2p.active,
            pid: p2p.pid,
        },
        safety: safety_status(),
    })
}

fn p2p_status(state: &AppState) -> P2pStatus {
    let mut p2p = state.p2p.lock().expect("p2p mutex poisoned");
    let mut active = false;
    let mut pid = None;
    if let Some(child) = p2p.child.as_mut() {
        pid = Some(child.id());
        match child.try_wait() {
            Ok(Some(status)) => {
                p2p.status = format!("exited: {status}");
                p2p.child = None;
            }
            Ok(None) => {
                active = true;
                if p2p.status == "disabled" {
                    p2p.status = "started".into();
                }
            }
            Err(e) => {
                p2p.status = format!("unknown: {e}");
            }
        }
    }
    P2pStatus {
        enabled: p2p.enabled,
        status: p2p.status.clone(),
        active,
        pid,
    }
}

fn safety_status() -> SafetyStatus {
    SafetyStatus {
        moves_funds: false,
        signs_bitcoin_transactions: false,
        broadcasts_transactions: false,
        stores_wallet_seeds_or_spending_keys: false,
        manages_signed_profiles: true,
    }
}

fn resolver_chain(home: &Path) -> ChainResolver {
    let mut chain = ChainResolver::new();
    if let Ok(registry) = Registry::open(home) {
        chain = chain.push(registry);
    }
    chain
        .push(Bip353Resolver::new())
        .push(HttpResolver::new())
        .push(NostrResolver)
}

fn build_methods(wallet: &WalletState, network: &str) -> Vec<PaymentMethod> {
    let mut methods = Vec::new();
    if let Some(addr) = &wallet.lightning_address {
        methods.push(PaymentMethod::Lightning {
            label: "Lightning Address".into(),
            lightning_address: Some(addr.clone()),
            lnurl: None,
            bolt12: None,
            receiver_pubkey: None,
        });
    }
    if let Some(addr) = &wallet.onchain_address {
        methods.push(PaymentMethod::Onchain {
            label: format!("Bitcoin ({})", network),
            network: bitcoin_network(network),
            address: addr.clone(),
            pubkey_hint: None,
            descriptor_hint: None,
        });
    }
    if let (Some(server), Some(pubkey)) = (&wallet.ark_server, &wallet.ark_pubkey) {
        methods.push(PaymentMethod::Ark {
            label: "Ark".into(),
            server: server.clone(),
            pubkey: pubkey.clone(),
            vtxo_pointer: None,
            proof: None,
            expires_at: None,
        });
    }
    methods
}

fn load_or_create_identity(home: &Path) -> Result<WalletState> {
    let mut wallet = load_wallet(home)?;
    if wallet.identity_pubkey.is_some() {
        return Ok(wallet);
    }
    let kp = generate_identity_keypair();
    let pubkey = hex::encode(kp.public_key.serialize());
    save_identity_key(home, &kp.secret_key)?;
    wallet.identity_pubkey = Some(pubkey);
    wallet.created_at = Some(now());
    wallet.updated_at = Some(now());
    save_wallet(home, &wallet)?;
    Ok(wallet)
}

fn load_wallet(home: &Path) -> Result<WalletState> {
    let path = wallet_path(home);
    if !path.exists() {
        return Ok(WalletState::default());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_wallet(home: &Path, wallet: &WalletState) -> Result<()> {
    fs::create_dir_all(home)?;
    let json = serde_json::to_string_pretty(wallet)?;
    assert_no_private_material(&json)?;
    fs::write(wallet_path(home), json)?;
    Ok(())
}

fn save_identity_key(home: &Path, secret_key: &secp256k1::SecretKey) -> Result<PathBuf> {
    let secp = secp256k1::Secp256k1::new();
    let pubkey = secp256k1::PublicKey::from_secret_key(&secp, secret_key);
    let dir = home.join(IDENTITY_SUBDIR);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.key", hex::encode(pubkey.serialize())));
    fs::write(&path, hex::encode(secret_key.secret_bytes()))?;
    set_owner_only(&path)?;
    Ok(path)
}

fn load_identity_key(home: &Path, identity_pubkey: &str) -> Result<secp256k1::SecretKey> {
    let path = home
        .join(IDENTITY_SUBDIR)
        .join(format!("{identity_pubkey}.key"));
    let hex_secret = fs::read_to_string(&path)
        .with_context(|| format!("reading identity key at {}", path.display()))?;
    let bytes = hex::decode(hex_secret.trim())?;
    let secret = secp256k1::SecretKey::from_slice(&bytes)?;
    let secp = secp256k1::Secp256k1::new();
    let actual = secp256k1::PublicKey::from_secret_key(&secp, &secret);
    if hex::encode(actual.serialize()) != identity_pubkey {
        anyhow::bail!("identity key file does not match wallet identity pubkey");
    }
    Ok(secret)
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

fn start_p2p_bridge(home: &Path, wallet: &WalletState) -> Result<(String, Child)> {
    let alias = wallet
        .alias
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no local alias/profile yet"))?;
    let signed = Registry::open(home)?.resolve_alias(alias)?.clone();
    if !verify_signed_profile(&signed)? {
        anyhow::bail!("refusing to bridge invalid signed profile");
    }
    let out_path = home.join(format!("{}-profile.json", sanitize(alias)));
    fs::write(&out_path, serde_json::to_string_pretty(&signed)?)?;

    let repo_root = std::env::current_dir()?;
    let sdk_dir = repo_root.join("sdk").join("satspath-p2p");
    let script = sdk_dir.join("examples").join("publish.mjs");
    if !script.exists() {
        anyhow::bail!("P2P SDK publish script not found at {}", script.display());
    }
    let child = Command::new("node")
        .arg("examples/publish.mjs")
        .arg(&out_path)
        .current_dir(&sdk_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("starting Node Holepunch bridge")?;

    // The child is intentionally detached from request handling. If it exits,
    // status remains "started"; users can see process logs by running the SDK
    // directly while this bridge is still optional.
    Ok((format!("started: publishing {alias}"), child))
}

fn print_startup_status(state: &AppState) -> Result<()> {
    let status = status_response(state)?;
    println!("satspathd node starting");
    println!("  bind: {}", status.bind);
    println!("  network: {}", status.network);
    println!("  home: {}", status.home);
    println!(
        "  identity: {}",
        status
            .identity_fingerprint
            .as_deref()
            .unwrap_or("(not initialized)")
    );
    println!(
        "  alias: {}",
        status.alias.as_deref().unwrap_or("(not configured)")
    );
    println!(
        "  methods: {}",
        if status.methods.is_empty() {
            "(none)".into()
        } else {
            status.methods.join(", ")
        }
    );
    println!("  p2p: {}", status.p2p.status);
    println!("  safety: profile node only; no funds moved, no Bitcoin tx signing, no broadcast");
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    if body.trim().is_empty() {
        anyhow::bail!("request body must be JSON");
    }
    Ok(serde_json::from_str(&body)?)
}

fn json_result<T: Serialize>(
    status: StatusCode,
    result: Result<T>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match result {
        Ok(value) => json_response(status, &value),
        Err(e) => json_error(StatusCode(500), e),
    }
}

// ─── Receive wallet UI ─────────────────────────────────────────────────────────

const INDEX_HTML: &str = include_str!("index.html");

#[derive(Debug, Serialize)]
struct ReceiveView {
    /// Masked alias, e.g. `r***@gmail.com` — the raw identifier is never exposed.
    alias: String,
    rail: String,
    payload: String,
    qr_svg: String,
}

/// Compute the wallet owner's preferred receive QR, entirely locally. Prefers
/// Lightning → on-chain → Ark. Returns a reusable (amount-less) receive pointer.
fn receive_view(state: &AppState) -> Result<ReceiveView> {
    let wallet = load_wallet(&state.home)?;
    let alias = wallet
        .alias
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no profile yet — set one via POST /v1/profile"))?;
    let methods = build_methods(&wallet, &state.network);
    let method = methods
        .iter()
        .find(|m| matches!(m, PaymentMethod::Lightning { .. }))
        .or_else(|| {
            methods
                .iter()
                .find(|m| matches!(m, PaymentMethod::Onchain { .. }))
        })
        .or_else(|| {
            methods
                .iter()
                .find(|m| matches!(m, PaymentMethod::Ark { .. }))
        })
        .ok_or_else(|| {
            anyhow::anyhow!("no receive methods — add one via POST /v1/profile/methods")
        })?;
    let payload = receive_payload_for(method)?;
    Ok(ReceiveView {
        alias: mask_identifier(&alias),
        rail: method.method_name().to_string(),
        qr_svg: qr_svg(&payload)?,
        payload,
    })
}

/// A public, amount-less receive pointer for a method.
fn receive_payload_for(method: &PaymentMethod) -> Result<String> {
    let payload = match method {
        PaymentMethod::Lightning {
            lightning_address: Some(addr),
            ..
        } => addr.clone(),
        PaymentMethod::Lightning {
            lnurl: Some(url), ..
        } => url.clone(),
        PaymentMethod::Onchain { address, .. } => format!("bitcoin:{address}"),
        PaymentMethod::Ark { server, pubkey, .. } => {
            format!("satspath:ark?server={server}&pubkey={pubkey}")
        }
        _ => anyhow::bail!("selected method has no receive pointer"),
    };
    assert_no_private_material(&payload)?;
    Ok(payload)
}

/// Render a payload as a self-contained black-and-white SVG QR.
fn qr_svg(data: &str) -> Result<String> {
    let code = QrCode::new(data.as_bytes()).map_err(|e| anyhow::anyhow!("QR encode: {e}"))?;
    let width = code.width();
    let colors = code.to_colors();
    let quiet = 4usize;
    let size = width + quiet * 2;
    let mut rects = String::new();
    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] == Color::Dark {
                rects.push_str(&format!(
                    "<rect x='{}' y='{}' width='1' height='1'/>",
                    x + quiet,
                    y + quiet
                ));
            }
        }
    }
    Ok(format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {size} {size}' \
         shape-rendering='crispEdges'><rect width='100%' height='100%' fill='#fff'/>\
         <g fill='#000'>{rects}</g></svg>"
    ))
}

fn html_response(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
        .expect("static header");
    Response::from_data(body.as_bytes().to_vec())
        .with_status_code(StatusCode(200))
        .with_header(header)
        .with_header(cors_origin_header())
}

/// Best-effort open of the default browser. Never fails the daemon.
fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let (cmd, args): (&str, Vec<&str>) = ("open", vec![url]);
    #[cfg(target_os = "linux")]
    let (cmd, args): (&str, Vec<&str>) = ("xdg-open", vec![url]);
    #[cfg(target_os = "windows")]
    let (cmd, args): (&str, Vec<&str>) = ("cmd", vec!["/C", "start", "", url]);
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let (cmd, args): (&str, Vec<&str>) = ("", vec![]);
    if !cmd.is_empty() {
        let _ = Command::new(cmd).args(args).spawn();
    }
}

fn json_response<T: Serialize>(
    status: StatusCode,
    value: &T,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body =
        serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{\"error\":\"json\"}".to_vec());
    Response::from_data(body)
        .with_status_code(status)
        .with_header(json_header())
        .with_header(cors_origin_header())
        .with_header(cors_methods_header())
        .with_header(cors_headers_header())
}

fn empty_response(status: StatusCode) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(Vec::new())
        .with_status_code(status)
        .with_header(cors_origin_header())
        .with_header(cors_methods_header())
        .with_header(cors_headers_header())
}

fn json_error(status: StatusCode, error: anyhow::Error) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        status,
        &ErrorResponse {
            error: error.to_string(),
        },
    )
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).expect("valid static header")
}

fn cors_origin_header() -> Header {
    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).expect("valid static header")
}

fn cors_methods_header() -> Header {
    Header::from_bytes(
        &b"Access-Control-Allow-Methods"[..],
        &b"GET, POST, PUT, OPTIONS"[..],
    )
    .expect("valid static header")
}

fn cors_headers_header() -> Header {
    Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type"[..])
        .expect("valid static header")
}

fn safety_warnings() -> Vec<&'static str> {
    vec![
        "satspathd does not move funds",
        "satspathd does not sign Bitcoin transactions",
        "satspathd does not broadcast transactions",
        "payment execution happens in an external wallet",
    ]
}

fn wallet_path(home: &Path) -> PathBuf {
    home.join(WALLET_FILE)
}

fn default_home() -> PathBuf {
    // Prefer a `.satspath/` in the current directory (e.g. a wallet created with
    // `satspath wallet ...`) so the daemon serves the same profile seamlessly;
    // otherwise fall back to the per-user `~/.satspath`.
    let local = PathBuf::from(".satspath");
    if local.is_dir() {
        return local;
    }
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".satspath")
    } else {
        local
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn bitcoin_network(network: &str) -> BitcoinNetwork {
    match network.to_ascii_lowercase().as_str() {
        "mainnet" | "bitcoin" => BitcoinNetwork::Mainnet,
        "regtest" => BitcoinNetwork::Regtest,
        // devnet uses testnet-form receive addresses until a distinct core
        // network enum is added.
        _ => BitcoinNetwork::Testnet,
    }
}

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn sanitize(alias: &str) -> String {
    alias
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_creation_persists_public_wallet_state_only() {
        let dir = tempfile::tempdir().unwrap();
        let wallet = load_or_create_identity(dir.path()).unwrap();
        assert!(wallet.identity_pubkey.is_some());
        let raw = fs::read_to_string(wallet_path(dir.path())).unwrap();
        assert!(!raw.contains("xprv"));
        assert!(!raw.contains("mnemonic"));
        assert!(!raw.contains("secret_key"));
    }

    #[test]
    fn profile_signing_writes_resolvable_signed_profile() {
        let dir = tempfile::tempdir().unwrap();
        let mut wallet = load_or_create_identity(dir.path()).unwrap();
        wallet.alias = Some("alice@example.com".into());
        wallet.lightning_address = Some("alice@example.com".into());
        sign_and_store(dir.path(), &mut wallet, "devnet").unwrap();

        let signed = Registry::open(dir.path())
            .unwrap()
            .resolve_alias("alice@example.com")
            .unwrap()
            .clone();
        assert!(verify_signed_profile(&signed).unwrap());
        assert_eq!(signed.profile.methods.len(), 1);
    }
}
