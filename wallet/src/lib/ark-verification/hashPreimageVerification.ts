/**
 * Hash Preimage Verification — HTLC/Submarine Swap support
 *
 * Adapted from ARK/src/hashPreimageVerification.ts for @noble/hashes v2.
 */

import { sha256 } from '@noble/hashes/sha2.js'
import { ripemd160 } from '@noble/hashes/legacy.js'
import { Script } from '@scure/btc-signer/script.js'
import { hex } from '@scure/base'
import type { DAGNode } from './vtxoDAGVerification'
import { VtxoVerificationError } from './vtxoDAGVerification'

// ─── Supported Hash Operations ────────────────────────────────────────────────

const HASH_OPS: Record<string, (preimage: Uint8Array) => Uint8Array> = {
  SHA256: (data) => sha256(data),
  HASH160: (data) => ripemd160(sha256(data)),
  HASH256: (data) => sha256(sha256(data)),
  RIPEMD160: (data) => ripemd160(data),
}

// ─── Types ───────────────────────────────────────────────────────────────────

export interface HashCondition {
  opcode: string
  expectedHash: Uint8Array
  opcodeIndex: number
}

export interface PreimageVerificationResult {
  valid: boolean
  opcode: string
  preimage: string
  computedHash: string
  expectedHash: string
}

// ─── Extraction ───────────────────────────────────────────────────────────────

export function extractHashConditions(decoded: (string | number | Uint8Array)[]): HashCondition[] {
  const conditions: HashCondition[] = []
  for (let i = 0; i < decoded.length; i++) {
    const op = decoded[i]
    if (typeof op === 'string' && op in HASH_OPS) {
      if (i + 1 < decoded.length && decoded[i + 1] instanceof Uint8Array) {
        const afterHash = i + 2 < decoded.length ? decoded[i + 2] : null
        if (afterHash === 'EQUAL' || afterHash === 'EQUALVERIFY') {
          conditions.push({ opcode: op, expectedHash: decoded[i + 1] as Uint8Array, opcodeIndex: i })
        }
      }
    }
  }
  return conditions
}

// ─── Verification ─────────────────────────────────────────────────────────────

export function verifyPreimage(preimage: Uint8Array, condition: HashCondition): PreimageVerificationResult {
  const hashFn = HASH_OPS[condition.opcode]
  if (!hashFn) {
    return {
      valid: false,
      opcode: condition.opcode,
      preimage: hex.encode(preimage),
      computedHash: '',
      expectedHash: hex.encode(condition.expectedHash),
    }
  }
  const computedHash = hashFn(preimage)
  const valid =
    computedHash.length === condition.expectedHash.length && computedHash.every((b, i) => b === condition.expectedHash[i])
  return {
    valid,
    opcode: condition.opcode,
    preimage: hex.encode(preimage),
    computedHash: hex.encode(computedHash),
    expectedHash: hex.encode(condition.expectedHash),
  }
}

export function verifyNodeHashPreimages(node: DAGNode, witnessPreimages?: Map<string, Uint8Array>): void {
  const input = node.tx.getInput(0)
  if (!input.tapLeafScript) return

  for (const leaf of input.tapLeafScript) {
    const [_cb, scriptWithVersion] = leaf
    if (!scriptWithVersion || scriptWithVersion.length < 2) continue

    const scriptBytes = scriptWithVersion.slice(0, -1)
    let decoded: (string | number | Uint8Array)[]
    try {
      decoded = Script.decode(scriptBytes)
    } catch {
      continue
    }

    const conditions = extractHashConditions(decoded)
    if (conditions.length === 0) continue

    for (const condition of conditions) {
      const hashHex = hex.encode(condition.expectedHash)
      if (!witnessPreimages || !witnessPreimages.has(hashHex)) {
        throw new VtxoVerificationError(
          `Hash condition (${condition.opcode}) in ${node.txid} but no preimage supplied`,
          'MISSING_HASH_PREIMAGE',
          { txid: node.txid, hash: hashHex },
        )
      }
      const preimage = witnessPreimages.get(hashHex)!
      const result = verifyPreimage(preimage, condition)
      if (!result.valid) {
        throw new VtxoVerificationError(
          `Hash preimage verification failed (${condition.opcode}) in tx ${node.txid}`,
          'INVALID_HASH_PREIMAGE',
          {
            txid: node.txid,
            opcode: condition.opcode,
            computedHash: result.computedHash,
            expectedHash: result.expectedHash,
          },
        )
      }
    }
  }
}

// ─── Entry Point ──────────────────────────────────────────────────────────────

export function verifyDAGHashPreimages(rootNode: DAGNode, witnessPreimages?: Map<string, Uint8Array>): void {
  const stack: DAGNode[] = [rootNode]
  while (stack.length > 0) {
    const node = stack.pop()!
    verifyNodeHashPreimages(node, witnessPreimages)
    for (const child of node.children.values()) stack.push(child)
  }
}
