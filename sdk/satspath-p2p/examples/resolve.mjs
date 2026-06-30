// Resolve a SatsPath alias peer-to-peer over the DHT. Run on the sender's machine
// (a different computer/network from the publisher):
//
//   node examples/resolve.mjs rodrigo@satspath.dev
//
// Prints the verified signed profile, or exits non-zero on failure/timeout.

import { SatsPathP2PNode } from "../src/index.js";

const args = process.argv.slice(2);
const json = args.includes("--json");
const timeoutIdx = args.indexOf("--timeout-ms");
const timeoutMs =
  timeoutIdx >= 0 && args[timeoutIdx + 1]
    ? Number.parseInt(args[timeoutIdx + 1], 10)
    : 20000;
const alias = args.find((arg) => !arg.startsWith("--") && arg !== String(timeoutMs));
if (!alias) {
  console.error("usage: node examples/resolve.mjs <alias> [--json] [--timeout-ms <ms>]");
  process.exit(1);
}

const node = new SatsPathP2PNode();
if (!json) {
  console.log("SatsPath P2P node address:", node.address);
  console.log(`Resolving ${alias} over Holepunch...`);
}

try {
  const signed = await node.resolve(alias, { timeoutMs });
  if (!json) console.log("✓ Resolved and signature-verified:");
  console.log(JSON.stringify(signed, null, 2));
} catch (e) {
  console.error(json ? e.message : `✗ ${e.message}`);
  process.exitCode = 1;
} finally {
  await node.destroy();
}
