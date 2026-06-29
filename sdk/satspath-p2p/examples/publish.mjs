// Serve a signed profile peer-to-peer. Run on the receiver's machine.
//
//   satspath export rodrigo@satspath.dev > rodrigo.json
//   node examples/publish.mjs rodrigo.json
//
// Leave it running; senders elsewhere can resolve the alias over the DHT.

import { readFileSync } from "node:fs";

import { SatsPathP2PNode } from "../src/index.js";

const file = process.argv[2];
if (!file) {
  console.error("usage: node examples/publish.mjs <signed-profile.json>");
  process.exit(1);
}

const signed = JSON.parse(readFileSync(file, "utf8"));
const node = new SatsPathP2PNode();

console.log("SatsPath P2P node address:", node.address);
const { topic } = await node.publish(signed);
console.log(`Publishing ${signed.profile.alias}`);
console.log("DHT topic:", topic);
console.log("Serving over Holepunch. Press Ctrl+C to stop.");

process.on("SIGINT", async () => {
  await node.destroy();
  process.exit(0);
});
