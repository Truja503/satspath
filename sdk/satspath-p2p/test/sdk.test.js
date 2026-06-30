import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { topicForAlias } from "../src/topic.js";
import { generateNodeKeyPair, addressFromPublicKey } from "../src/identity.js";
import { verifySignedProfile, canonicalAlias } from "../src/profile.js";

const fixture = JSON.parse(
  readFileSync(new URL("../test-fixture.json", import.meta.url), "utf8"),
);

test("topic is deterministic and 32 bytes", () => {
  const a = topicForAlias("rodrigo@satspath.dev");
  const b = topicForAlias("RODRIGO@satspath.dev "); // case/space-insensitive
  assert.equal(a.length, 32);
  assert.deepEqual(a, b);
});

test("different aliases get different topics", () => {
  assert.notDeepEqual(
    topicForAlias("alice@example.com"),
    topicForAlias("bob@example.com"),
  );
});

test("node keypair is random (a bit different each time)", () => {
  const a = addressFromPublicKey(generateNodeKeyPair().publicKey);
  const b = addressFromPublicKey(generateNodeKeyPair().publicKey);
  assert.equal(a.length, 64); // 32-byte ed25519 pubkey, hex
  assert.notEqual(a, b);
});

test("verifies a real Rust-signed profile", () => {
  // Proves JS canonicalization matches Rust's serde_json::to_string(profile).
  assert.equal(verifySignedProfile(fixture), true);
});

test("rejects a tampered profile (alias changed after signing)", () => {
  const tampered = JSON.parse(JSON.stringify(fixture));
  tampered.profile.alias = "evil@hacker.com";
  assert.equal(verifySignedProfile(tampered), false);
});

test("rejects a tampered signature", () => {
  const tampered = JSON.parse(JSON.stringify(fixture));
  tampered.signature = tampered.signature.slice(0, -2) + "00";
  assert.equal(verifySignedProfile(tampered), false);
});

test("rejects malformed input without throwing", () => {
  assert.equal(verifySignedProfile(null), false);
  assert.equal(verifySignedProfile({}), false);
  assert.equal(verifySignedProfile({ profile: {}, signature: "zz" }), false);
});

test("canonicalAlias normalizes case and whitespace", () => {
  assert.equal(canonicalAlias("  Rodrigo@Satspath.DEV "), "rodrigo@satspath.dev");
});
