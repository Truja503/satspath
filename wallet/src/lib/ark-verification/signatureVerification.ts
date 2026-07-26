/**
 * VTXO Signature Verification — Schnorr & MuSig2
 *
 * Adapted from ARK/src/signatureVerification.ts for @scure/* v2 + @noble/* v2.
 */

import { schnorr } from '@noble/curves/secp256k1.js'
import { hex } from '@scure/base'
import { taprootTweakPubkey } from '@scure/btc-signer/utils.js'
import type { DAGNode } from './vtxoDAGVerification'
import { VtxoVerificationError } from './vtxoDAGVerification'

const SIGHASH_DEFAULT = 0x00

export function verifyDAGSignatures(node: DAGNode): void {
  const stack: DAGNode[] = [node]
  while (stack.length > 0) {
    const current = stack.pop()!
    verifyNodeSignature(current)
    for (const child of current.children.values()) stack.push(child)
  }
}

export function verifyNodeSignature(node: DAGNode): void {
  const { tx, txid } = node
  const input = tx.getInput(0)
  const tapKeySig = input.tapKeySig
  const tapInternalKey = input.tapInternalKey

  if (!tapKeySig) {
    if (input.tapScriptSig && input.tapScriptSig.length > 0) {
      return verifyNodeScriptPathSignature(node)
    }
    throw new VtxoVerificationError(`Tx ${txid} missing tapKeySig`, 'MISSING_SIGNATURE', { txid })
  }

  if (!tapInternalKey) {
    throw new VtxoVerificationError(`Tx ${txid} missing tapInternalKey`, 'MISSING_INTERNAL_KEY', { txid })
  }

  let signature = tapKeySig
  let sighashType = SIGHASH_DEFAULT

  if (tapKeySig.length === 65) {
    signature = tapKeySig.slice(0, 64)
    sighashType = tapKeySig[64]
    if (sighashType !== 0x01) {
      throw new VtxoVerificationError(
        `Tx ${txid} unsupported sighash: 0x${sighashType.toString(16)}`,
        'UNSUPPORTED_SIGHASH',
        { txid, sighashType },
      )
    }
  } else if (tapKeySig.length !== 64) {
    throw new VtxoVerificationError(
      `Tx ${txid} invalid signature length (${tapKeySig.length})`,
      'INVALID_SIGNATURE_LENGTH',
      { txid, length: tapKeySig.length },
    )
  }

  const prevOuts = getPrevOutsForNode(node)
  const prevScripts = prevOuts.map((o) => o.script)
  const prevAmounts = prevOuts.map((o) => o.amount)

  // preimageWitnessV1 is available on the Transaction instance
  const sighash = (tx as unknown as {
    preimageWitnessV1(
      idx: number,
      prevOutScript: Uint8Array[],
      hashType: number,
      amount: bigint[],
    ): Uint8Array
  }).preimageWitnessV1(0, prevScripts, sighashType, prevAmounts)

  const merkleRoot = input.tapMerkleRoot || new Uint8Array(0)
  const [tweakedKey] = taprootTweakPubkey(tapInternalKey, merkleRoot)

  const isValid = schnorr.verify(signature, sighash, tweakedKey)
  if (!isValid) {
    throw new VtxoVerificationError(`Invalid signature for tx ${txid}`, 'INVALID_SIGNATURE', {
      txid,
      sighashType,
      internalKey: hex.encode(tapInternalKey),
      tweakedKey: hex.encode(tweakedKey),
    })
  }
}

function verifyNodeScriptPathSignature(node: DAGNode): void {
  throw new VtxoVerificationError(
    'Script-path spends not yet implemented in client verification',
    'UNSUPPORTED_SPEND_PATH',
    { txid: node.txid },
  )
}

function getPrevOutsForNode(node: DAGNode): { script: Uint8Array; amount: bigint }[] {
  if (!node.ancestor) {
    const context = (node as DAGNode & { prevOutContext?: { script: Uint8Array; amount: bigint } }).prevOutContext
    if (!context) throw new Error('Commitment output context missing for anchoring node')
    return [context]
  }
  const ancestorNode = node.ancestor
  const ancestorOutput = ancestorNode.tx.getOutput(node.ancestorOutputIndex ?? 0)
  if (!ancestorOutput.script || ancestorOutput.amount === undefined) {
    throw new Error('Ancestor output info missing for sighash calculation')
  }
  return [{ script: ancestorOutput.script, amount: ancestorOutput.amount }]
}
