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
export {};
//# sourceMappingURL=index.d.ts.map