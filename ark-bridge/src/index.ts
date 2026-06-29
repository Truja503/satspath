/**
 * satspath-ark-bridge — JSON-RPC bridge over stdin/stdout
 *
 * Exposes the ARK client-side validation SDK to satspath-swaps (Rust).
 *
 * Protocol:
 *   - Rust writes one JSON line to stdin per request
 *   - Bridge writes one JSON line to stdout per response
 *   - Bridge writes diagnostic logs to stderr (never stdout)
 *
 * Request format:
 *   { "id": 1, "method": "verifyVtxo",        "params": { ... } }
 *   { "id": 2, "method": "onReceiveVtxo",      "params": { ... } }
 *   { "id": 3, "method": "executeSovereignExit","params": { ... } }
 *   { "id": 4, "method": "getBalance",          "params": { ... } }
 *   { "id": 5, "method": "ping",                "params": {} }
 *
 * Response format (success):
 *   { "id": 1, "result": { ... } }
 *
 * Response format (error):
 *   { "id": 1, "error": { "code": -1, "message": "..." } }
 */

import * as readline from "node:readline";
import { ArkdIndexerProvider } from "./arkdProvider.js";
import { BitcoinRpcProvider } from "./bitcoinRpc.js";
import {
  reconstructAndValidateVtxoDAG,
  verifyVtxoComplete,
} from "./vtxoDAGVerification.js";
import {
  onReceiveVtxo,
  executeSovereignExit,
  setStorageMasterKey,
  getBroadcastSequence,
} from "./sovereignStorage.js";
import { MockWalletAuthenticator } from "./authenticator.js";

// ─── In-memory StorageProvider ───────────────────────────────────────────────
// For production, replace with encrypted file storage (LocalStorage equivalent for Node.js).

const inMemoryStore = new Map<string, string>();

const storageProvider = {
  async setItem(key: string, value: string): Promise<void> {
    inMemoryStore.set(key, value);
    log(`[store] SET ${key} (${value.length} bytes)`);
  },
  async getItem(key: string): Promise<string | null> {
    return inMemoryStore.get(key) ?? null;
  },
  async removeItem(key: string): Promise<void> {
    inMemoryStore.delete(key);
  },
};

// ─── Logging (stderr only — stdout is reserved for JSON-RPC) ─────────────────

function log(msg: string): void {
  process.stderr.write(`[ark-bridge] ${msg}\n`);
}

// ─── Provider factory ─────────────────────────────────────────────────────────

interface ProviderConfig {
  arkd_url?: string;
  btc_rpc_url?: string;
  btc_rpc_user?: string;
  btc_rpc_pass?: string;
}

function makeProviders(cfg: ProviderConfig) {
  const indexer = new ArkdIndexerProvider(cfg.arkd_url ?? "http://localhost:18080");
  const onchain = new BitcoinRpcProvider(
    cfg.btc_rpc_url ?? "http://localhost:18443",
    cfg.btc_rpc_user ?? "user",
    cfg.btc_rpc_pass ?? "password"
  );
  return { indexer, onchain };
}

// ─── Method handlers ──────────────────────────────────────────────────────────

/**
 * ping — health check.
 */
async function handlePing(_params: Record<string, unknown>): Promise<{ pong: true }> {
  return { pong: true };
}

/**
 * initKey — initialize the storage master key from a password.
 * Must be called before any storage operations.
 */
async function handleInitKey(params: {
  password: string;
}): Promise<{ ok: true }> {
  const salt = Buffer.alloc(32, 0x55); // For MVP: stable salt. Production: user-specific salt from profile.
  const key = MockWalletAuthenticator.deriveMasterKey(params.password, salt);
  setStorageMasterKey(key);
  log("Master key initialized from password.");
  return { ok: true };
}

/**
 * verifyVtxo — run the full VTXO DAG verification pipeline.
 *
 * Used by satspath-swaps before:
 *   - Accepting a VTXO as input for a Chain Swap (Ark→L1)
 *   - Routing an Ark payment
 */
async function handleVerifyVtxo(params: {
  txid: string;
  vout: number;
  min_confirmations?: number;
} & ProviderConfig): Promise<{
  valid: boolean;
  commitment_txid: string;
  vtxo_root_txid: string;
  diagnostics: string[];
}> {
  const { indexer, onchain } = makeProviders(params);

  const result = await verifyVtxoComplete(
    { txid: params.txid, vout: params.vout },
    indexer,
    onchain,
    params.min_confirmations ?? 1
  );

  return {
    valid: true,
    commitment_txid: result.commitmentTxid,
    vtxo_root_txid: result.vtxoRoot.txid,
    diagnostics: result.diagnostics,
  };
}

/**
 * onReceiveVtxo — full pipeline: verify + persist sovereign exit data.
 *
 * Called by satspath-swaps when:
 *   - A reverse swap (LN→Ark) completes and a new VTXO is received
 *   - A user receives an Ark payment
 *
 * After this call, the user can exit sovereignly without the ASP.
 */
async function handleOnReceiveVtxo(params: {
  txid: string;
  vout: number;
} & ProviderConfig): Promise<{
  success: boolean;
  diagnostics: string[];
  error?: string;
}> {
  const { indexer, onchain } = makeProviders(params);

  const result = await onReceiveVtxo(
    { txid: params.txid, vout: params.vout },
    indexer,
    onchain,
    storageProvider
  );

  return result;
}

/**
 * executeSovereignExit — broadcast the pre-stored exit sequence without ASP.
 *
 * Called by satspath-swaps when:
 *   - The user wants to force-exit from Ark (unilateral exit)
 *   - The ASP is offline or unresponsive
 *
 * Uses ONLY locally stored, pre-verified data — no network queries to arkd.
 */
async function handleExecuteSovereignExit(params: {
  vtxo_txid: string;
} & ProviderConfig): Promise<{
  success: boolean;
  broadcasted_txids: string[];
  error?: string;
}> {
  const { onchain } = makeProviders(params);

  const result = await executeSovereignExit(
    params.vtxo_txid,
    storageProvider,
    onchain
  );

  return {
    success: result.success,
    broadcasted_txids: result.broadcastedTxids,
    error: result.error,
  };
}

/**
 * getBroadcastSequence — retrieve the stored exit transaction sequence.
 *
 * Useful for inspection/debugging without broadcasting.
 */
async function handleGetBroadcastSequence(params: {
  vtxo_txid: string;
}): Promise<{
  sequence: string[];
  count: number;
}> {
  const sequence = await getBroadcastSequence(params.vtxo_txid, storageProvider);
  return { sequence, count: sequence.length };
}

/**
 * validateDag — lighter-weight DAG-only verification (no on-chain anchoring check).
 *
 * Useful for quick structural validation before creating swaps.
 */
async function handleValidateDag(params: {
  txid: string;
  vout: number;
} & ProviderConfig): Promise<{
  valid: boolean;
  commitment_txid: string;
  chain_length: number;
  diagnostics: string[];
}> {
  const { indexer, onchain } = makeProviders(params);

  const result = await reconstructAndValidateVtxoDAG(
    { txid: params.txid, vout: params.vout },
    indexer,
    onchain
  );

  return {
    valid: true,
    commitment_txid: result.commitmentTxid,
    chain_length: result.diagnostics.filter((d: string) => d.startsWith("  ✓")).length,
    diagnostics: result.diagnostics,
  };
}

// ─── Dispatch table ───────────────────────────────────────────────────────────

const METHODS: Record<string, (params: any) => Promise<unknown>> = {
  ping: handlePing,
  initKey: handleInitKey,
  verifyVtxo: handleVerifyVtxo,
  validateDag: handleValidateDag,
  onReceiveVtxo: handleOnReceiveVtxo,
  executeSovereignExit: handleExecuteSovereignExit,
  getBroadcastSequence: handleGetBroadcastSequence,
};

// ─── JSON-RPC main loop ───────────────────────────────────────────────────────

interface RpcRequest {
  id: number | string;
  method: string;
  params: Record<string, unknown>;
}

interface RpcSuccess {
  id: number | string;
  result: unknown;
}

interface RpcError {
  id: number | string;
  error: { code: number; message: string };
}

function respond(resp: RpcSuccess | RpcError): void {
  process.stdout.write(JSON.stringify(resp) + "\n");
}

async function dispatch(req: RpcRequest): Promise<void> {
  const handler = METHODS[req.method];

  if (!handler) {
    respond({
      id: req.id,
      error: { code: -32601, message: `Method not found: ${req.method}` },
    });
    return;
  }

  try {
    const result = await handler(req.params ?? {});
    respond({ id: req.id, result });
  } catch (err: any) {
    log(`[error] method=${req.method} id=${req.id} error=${err.message}`);
    respond({
      id: req.id,
      error: { code: -1, message: err.message ?? String(err) },
    });
  }
}

// ─── Startup ──────────────────────────────────────────────────────────────────

log("satspath-ark-bridge started. Listening on stdin for JSON-RPC commands.");

const rl = readline.createInterface({
  input: process.stdin,
  terminal: false,
});

rl.on("line", async (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;

  let req: RpcRequest;
  try {
    req = JSON.parse(trimmed);
  } catch {
    // Malformed JSON — write error response with id=null
    respond({ id: "null", error: { code: -32700, message: "Parse error" } });
    return;
  }

  await dispatch(req);
});

rl.on("close", () => {
  log("stdin closed — bridge shutting down.");
  process.exit(0);
});
