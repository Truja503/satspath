/**
 * satspath-ark-bridge — JSON-RPC bridge over stdin/stdout
 *
 * Engine v0 status: this is a protocol-compatible stub bridge.
 * All methods are handled but VTXO DAG validation is not implemented.
 * The full ARK SDK integration is deferred to Engine v1.
 *
 * Protocol:
 *   - Rust writes one JSON line to stdin per request
 *   - Bridge writes one JSON line to stdout per response
 *   - Bridge writes diagnostic logs to stderr (never stdout)
 *
 * Request format:
 *   { "id": 1, "method": "verifyVtxo",         "params": { ... } }
 *   { "id": 2, "method": "onReceiveVtxo",       "params": { ... } }
 *   { "id": 3, "method": "executeSovereignExit", "params": { ... } }
 *   { "id": 4, "method": "getBalance",           "params": { ... } }
 *   { "id": 5, "method": "ping",                 "params": {} }
 *   { "id": 6, "method": "initKey",              "params": { "password": "..." } }
 *
 * Response format (success):  { "id": 1, "result": { ... } }
 * Response format (error):    { "id": 1, "error": { "code": -1, "message": "..." } }
 */
import * as readline from "node:readline";
// ─── Logging (stderr only — stdout is reserved for JSON-RPC) ─────────────────
function log(msg) {
    process.stderr.write(`[ark-bridge] ${msg}\n`);
}
function ok(id, result) {
    return { id, result };
}
function err(id, code, message) {
    return { id, error: { code, message } };
}
// ─── In-memory key storage (Engine v0) ───────────────────────────────────────
const initDone = new Set();
// ─── Method handlers ──────────────────────────────────────────────────────────
function handlePing(id) {
    return ok(id, { pong: true });
}
function handleInitKey(id, params) {
    const password = params["password"];
    if (typeof password !== "string" || password.length === 0) {
        return err(id, -32602, "initKey requires a non-empty 'password' field");
    }
    initDone.add("initialized");
    log("initKey: key slot initialized (Engine v0 stub — no real key derivation)");
    return ok(id, { success: true });
}
function handleVerifyVtxo(id, params) {
    const txid = params["txid"];
    const vout = params["vout"];
    if (typeof txid !== "string") {
        return err(id, -32602, "verifyVtxo requires 'txid' (string)");
    }
    // Engine v0: VTXO DAG validation not implemented.
    // Full implementation requires the ARK SDK (Engine v1).
    log(`verifyVtxo stub: txid=${txid} vout=${vout ?? 0} — not validated in Engine v0`);
    return ok(id, {
        valid: false,
        commitment_txid: "",
        vtxo_root_txid: "",
        diagnostics: [
            "VTXO DAG validation not implemented in Engine v0.",
            "Full ARK SDK integration is deferred to Engine v1.",
            `Received VTXO pointer: ${txid}:${vout ?? 0}`,
        ],
    });
}
function handleOnReceiveVtxo(id, params) {
    const txid = params["txid"];
    if (typeof txid !== "string") {
        return err(id, -32602, "onReceiveVtxo requires 'txid' (string)");
    }
    log(`onReceiveVtxo stub: txid=${txid} — not stored in Engine v0`);
    return ok(id, {
        success: false,
        diagnostics: [
            "Sovereign storage not implemented in Engine v0.",
            "VTXO received but not validated or stored.",
            "Engine v1 will implement full sovereign exit and storage.",
        ],
        error: "Engine v0 stub: no-op",
    });
}
function handleExecuteSovereignExit(id, _params) {
    log("executeSovereignExit: not implemented in Engine v0");
    return err(id, -32601, "executeSovereignExit is not implemented in Engine v0. " +
        "Sovereign exit requires PSBT construction (Engine v1 work).");
}
function handleGetBalance(id, _params) {
    log("getBalance stub: returning zero (Engine v0)");
    return ok(id, {
        onchain_sats: 0,
        vtxo_sats: 0,
        note: "Balance tracking not implemented in Engine v0.",
    });
}
// ─── Dispatcher ───────────────────────────────────────────────────────────────
function dispatch(req) {
    log(`→ ${req.method} (id=${req.id})`);
    try {
        switch (req.method) {
            case "ping": return handlePing(req.id);
            case "initKey": return handleInitKey(req.id, req.params ?? {});
            case "verifyVtxo": return handleVerifyVtxo(req.id, req.params ?? {});
            case "onReceiveVtxo": return handleOnReceiveVtxo(req.id, req.params ?? {});
            case "executeSovereignExit": return handleExecuteSovereignExit(req.id, req.params ?? {});
            case "getBalance": return handleGetBalance(req.id, req.params ?? {});
            default:
                return err(req.id, -32601, `Method not found: ${req.method}`);
        }
    }
    catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        log(`Error handling ${req.method}: ${msg}`);
        return err(req.id, -32603, `Internal error: ${msg}`);
    }
}
// ─── Main loop ────────────────────────────────────────────────────────────────
log("ARK bridge starting (Engine v0 stub — VTXO validation not implemented)");
const rl = readline.createInterface({
    input: process.stdin,
    terminal: false,
});
rl.on("line", (line) => {
    const trimmed = line.trim();
    if (!trimmed)
        return;
    let req;
    try {
        req = JSON.parse(trimmed);
    }
    catch {
        const resp = err(0, -32700, "Parse error: invalid JSON");
        process.stdout.write(JSON.stringify(resp) + "\n");
        return;
    }
    const resp = dispatch(req);
    log(`← ${req.method} (id=${req.id}) → ${JSON.stringify(resp)}`);
    process.stdout.write(JSON.stringify(resp) + "\n");
});
rl.on("close", () => {
    log("stdin closed — bridge exiting");
    process.exit(0);
});
//# sourceMappingURL=index.js.map