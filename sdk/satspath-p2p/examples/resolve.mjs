// Resolve a SatsPath alias peer-to-peer over the DHT. Run on the sender's machine
// (a different computer/network from the publisher):
//
//   node examples/resolve.mjs rodrigo@satspath.dev
//
// Prints the verified signed profile, or exits non-zero on failure/timeout.

import { SatsPathP2PNode } from "../src/index.js";

const alias = process.argv[2];
if (!alias) {
  console.error("usage: node examples/resolve.mjs <alias>");
  process.exit(1);
}

const node = new SatsPathP2PNode();
console.log("SatsPath P2P node address:", node.address);
console.log(`Resolving ${alias} over Holepunch...`);

try {
  const signed = await node.resolve(alias, { timeoutMs: 20000 });
  console.log("✓ Resolved and signature-verified:");
  console.log(JSON.stringify(signed, null, 2));
} catch (e) {
  console.error("✗", e.message);
  process.exitCode = 1;
} finally {
  await node.destroy();
}
