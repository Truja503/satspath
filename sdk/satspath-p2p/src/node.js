// SatsPathP2PNode — publish/resolve signed payment profiles over Holepunch.
//
// Uses Hyperswarm (HyperDHT) for DHT discovery + NAT hole-punching. A receiver
// `publish()`es their signed profile on the topic derived from their alias; a
// sender `resolve()`s the same alias, connects through the DHT, and receives the
// profile — which is **verified by signature** before being returned.
//
// Safety: this layer only moves public, signed payment profiles. It never moves
// funds, signs Bitcoin transactions, or broadcasts anything.

import Hyperswarm from "hyperswarm";

import { topicForAlias } from "./topic.js";
import { canonicalAlias, verifySignedProfile } from "./profile.js";
import { addressFromPublicKey } from "./identity.js";

export class SatsPathP2PNode {
  /**
   * @param {object} [opts]
   * @param {{ publicKey: Uint8Array, secretKey: Uint8Array }} [opts.keyPair]
   *        Persisted node identity. Omit to auto-generate a random one.
   * @param {object} [opts.swarm] Extra options forwarded to Hyperswarm.
   */
  constructor(opts = {}) {
    this.swarm = new Hyperswarm({ keyPair: opts.keyPair, ...opts.swarm });
    /** @type {Map<string, object>} topicHex -> signed profile we serve */
    this._serving = new Map();
    this.swarm.on("connection", (conn) => this._onServerConnection(conn));
  }

  /** This node's auto-generated random address (hex public key). */
  get address() {
    return addressFromPublicKey(this.swarm.keyPair.publicKey);
  }

  // When a peer connects, push every profile we're serving (each framed).
  _onServerConnection(conn) {
    conn.on("error", () => {});
    for (const signed of this._serving.values()) {
      writeFrame(conn, new TextEncoder().encode(JSON.stringify(signed)));
    }
  }

  /**
   * Announce a signed profile on its alias topic and serve it to peers.
   * @param {object} signed `{ profile, signature }` from `satspath export`.
   * @returns {Promise<{ topic: string, address: string }>}
   */
  async publish(signed) {
    if (!verifySignedProfile(signed)) {
      throw new Error("refusing to publish a profile with an invalid signature");
    }
    const topic = topicForAlias(signed.profile.alias);
    this._serving.set(hex(topic), signed);
    const discovery = this.swarm.join(topic, { server: true, client: false });
    await discovery.flushed();
    // Ensure the DHT announce has fully propagated before returning.
    await this.swarm.flush();
    return { topic: hex(topic), address: this.address };
  }

  /**
   * Resolve an alias over the DHT and return its verified signed profile.
   * @param {string} alias
   * @param {object} [opts]
   * @param {number} [opts.timeoutMs=15000]
   * @returns {Promise<object>} verified `{ profile, signature }`
   */
  async resolve(alias, opts = {}) {
    const timeoutMs = opts.timeoutMs ?? 15000;
    const topic = topicForAlias(alias);

    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        cleanup();
        reject(new Error(`resolve("${alias}") timed out after ${timeoutMs}ms`));
      }, timeoutMs);

      const onConn = (conn) => {
        conn.on("error", () => {});
        readFrame(conn)
          .then((buf) => {
            let signed;
            try {
              signed = JSON.parse(new TextDecoder().decode(buf));
            } catch {
              return; // ignore malformed; wait for another peer
            }
            // Trust comes from the signature, not the transport.
            if (!verifySignedProfile(signed)) return;
            if (canonicalAlias(signed.profile.alias) !== canonicalAlias(alias)) return;
            cleanup();
            resolve(signed);
          })
          .catch(() => {});
      };

      const cleanup = () => {
        clearTimeout(timer);
        this.swarm.off("connection", onConn);
      };

      // Attach the listener BEFORE joining so a connection that fires during the
      // DHT lookup/flush is never missed.
      this.swarm.on("connection", onConn);
      const discovery = this.swarm.join(topic, { client: true, server: false });
      discovery
        .flushed()
        .then(() => this.swarm.flush())
        .catch(() => {});
    });
  }

  /** Tear down the swarm and all DHT connections. */
  async destroy() {
    await this.swarm.destroy();
  }
}

// ─── length-prefixed framing (4-byte LE length + payload) ─────────────────────

function writeFrame(conn, payload) {
  const len = new Uint8Array(4);
  new DataView(len.buffer).setUint32(0, payload.length, true);
  conn.write(len);
  conn.write(payload);
}

function readFrame(conn) {
  return new Promise((resolve, reject) => {
    let acc = new Uint8Array(0);
    let need = -1;
    const onData = (data) => {
      acc = concat(acc, data);
      if (need < 0 && acc.length >= 4) {
        need = new DataView(acc.buffer, acc.byteOffset, 4).getUint32(0, true);
        acc = acc.subarray(4);
      }
      if (need >= 0 && acc.length >= need) {
        conn.off("data", onData);
        conn.off("error", onErr);
        resolve(acc.subarray(0, need));
      }
    };
    const onErr = (e) => {
      conn.off("data", onData);
      reject(e);
    };
    conn.on("data", onData);
    conn.on("error", onErr);
  });
}

function concat(a, b) {
  const out = new Uint8Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}

function hex(bytes) {
  return Buffer.from(bytes).toString("hex");
}
