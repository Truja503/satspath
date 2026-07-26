/**
 * Taproot Verification — BIP 341/342
 *
 * Adapted from ARK/src/taprootVerification.ts for @scure/* v2.
 */

import { hex } from '@scure/base'
import { taprootTweakPubkey, tagSchnorr, compareBytes } from '@scure/btc-signer/utils.js'
import { tapLeafHash } from '@scure/btc-signer/payment.js'
import { Script } from '@scure/btc-signer/script.js'
import { sha256 } from '@noble/hashes/sha2.js'
import type { DAGNode } from './vtxoDAGVerification'
import { VtxoVerificationError } from './vtxoDAGVerification'

function computeNodeTxid(node: DAGNode): string {
  const rawBytes = node.tx.toBytes(true, false)
  const hash1 = sha256(rawBytes)
  const hash2 = sha256(hash1)
  const reversed = new Uint8Array(hash2)
  reversed.reverse()
  return hex.encode(reversed)
}

function tapBranchHash(a: Uint8Array, b: Uint8Array): Uint8Array {
  let [left, right] = [a, b]
  if (compareBytes(b, a) === -1) [left, right] = [b, a]
  return tagSchnorr('TapBranch', left, right)
}

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false
  return true
}

export function verifyNodeTaproot(node: DAGNode): void {
  const input = node.tx.getInput(0)
  const witnessUtxo = input.witnessUtxo
  const internalKey = input.tapInternalKey
  const merkleRoot = input.tapMerkleRoot

  if (!internalKey) {
    throw new VtxoVerificationError(
      `Tx ${computeNodeTxid(node)} missing tapInternalKey (BIP 341)`,
      'MISSING_TAPROOT_METADATA',
    )
  }

  // 1. Verify tweaked pubkey matches the output script
  if (witnessUtxo?.script) {
    const rootBytes = merkleRoot || new Uint8Array(0)
    const [tweakedKey] = taprootTweakPubkey(internalKey, rootBytes)
    const expectedScript = new Uint8Array([0x51, 0x20, ...tweakedKey])
    if (!equalBytes(witnessUtxo.script, expectedScript)) {
      throw new VtxoVerificationError(
        `Invalid Taproot tweak for tx ${computeNodeTxid(node)}`,
        'INVALID_TAPROOT_TWEAK',
      )
    }
  }

  // 2. Validate Merkle proofs and exit policies
  if (merkleRoot && input.tapLeafScript) {
    for (const leaf of input.tapLeafScript) {
      const [cb, scriptWithVersion] = leaf
      if (!cb || !scriptWithVersion || scriptWithVersion.length < 1) continue

      const script = scriptWithVersion.slice(0, -1)
      const leafVersion = scriptWithVersion[scriptWithVersion.length - 1]

      verifyMerkleProof(merkleRoot, script, cb, computeNodeTxid(node), leafVersion)
      verifyArkExitPolicy(script, computeNodeTxid(node))
    }
  }
}

function verifyMerkleProof(
  merkleRoot: Uint8Array,
  script: Uint8Array,
  cb: unknown,
  txid: string,
  providedVersion: number,
): void {
  let controlBlock: Uint8Array

  if (cb instanceof Uint8Array) {
    controlBlock = cb
  } else if (
    cb !== null &&
    typeof cb === 'object' &&
    'internalKey' in cb &&
    'merklePath' in cb
  ) {
    const cbObj = cb as { internalKey: Uint8Array; merklePath: Uint8Array[] }
    const leafVersion = providedVersion & 0xfe
    const leafHash = tapLeafHash(script, leafVersion)
    let currentHash = leafHash
    for (const branch of cbObj.merklePath) currentHash = tapBranchHash(currentHash, branch)
    if (!equalBytes(currentHash, merkleRoot)) {
      throw new VtxoVerificationError(`Merkle proof failure in tx ${txid}`, 'INVALID_MERKLE_PROOF')
    }
    return
  } else {
    throw new VtxoVerificationError(`Invalid control block format in ${txid}`, 'INVALID_MERKLE_PROOF')
  }

  if (controlBlock.length < 33) {
    throw new VtxoVerificationError(`Invalid control block length in ${txid}`, 'INVALID_MERKLE_PROOF')
  }

  const leafVersion = controlBlock[0] & 0xfe
  const leafHash = tapLeafHash(script, leafVersion)
  let currentHash = leafHash
  const numSteps = (controlBlock.length - 33) / 32

  for (let i = 0; i < numSteps; i++) {
    const branch = controlBlock.slice(33 + i * 32, 33 + (i + 1) * 32)
    currentHash = tapBranchHash(currentHash, branch)
  }

  if (!equalBytes(currentHash, merkleRoot)) {
    throw new VtxoVerificationError(`Merkle proof failure in tx ${txid}`, 'INVALID_MERKLE_PROOF')
  }
}

function verifyArkExitPolicy(script: Uint8Array, txid: string): void {
  let decoded: (string | number | Uint8Array)[]
  try {
    decoded = Script.decode(script)
  } catch {
    throw new VtxoVerificationError(`Failed to decode tapleaf script in ${txid}`, 'INVALID_ARK_SCRIPT')
  }

  if (decoded.length === 1 && (decoded[0] === 1 || decoded[0] === 'TRUE')) {
    throw new VtxoVerificationError(`Forbidden trivial script (OP_TRUE) in ${txid}`, 'SECURITY_VIOLATION')
  }

  const hasCSV = decoded.some((op) => op === 'CHECKSEQUENCEVERIFY')
  const hasCheckSig = decoded.some((op) => op === 'CHECKSIG' || op === 'CHECKSIGVERIFY')
  const hasHash = decoded.some((op) => op === 'HASH160' || op === 'SHA256' || op === 'HASH256' || op === 'RIPEMD160')
  const hasCLTV = decoded.some((op) => op === 'CHECKLOCKTIMEVERIFY')

  const isArkStandard = hasCSV && hasCheckSig
  const isSwapClaim = hasHash && hasCheckSig
  const isSwapRefund = hasCLTV && hasCheckSig

  if (!isArkStandard && !isSwapClaim && !isSwapRefund) {
    throw new VtxoVerificationError(
      `Tapleaf script in ${txid} does not follow Ark or HTLC exit policies`,
      'INVALID_ARK_SCRIPT',
    )
  }
}
