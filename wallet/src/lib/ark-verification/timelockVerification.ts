/**
 * Timelock Verification — BIP 65/68/112
 *
 * Adapted from ARK/src/timelockVerification.ts for @scure/* v2.
 */

import { Script } from '@scure/btc-signer/script.js'
import type { DAGNode } from './vtxoDAGVerification'
import { VtxoVerificationError } from './vtxoDAGVerification'

// ─── Constants ────────────────────────────────────────────────────────────────
const SEQUENCE_LOCKTIME_DISABLE_FLAG = 1 << 31
const SEQUENCE_LOCKTIME_TYPE_FLAG = 1 << 22
const SEQUENCE_LOCKTIME_MASK = 0x0000ffff
const SEQUENCE_FINAL = 0xffffffff
const LOCKTIME_THRESHOLD = 500_000_000

// ─── Types ───────────────────────────────────────────────────────────────────

export interface TimelockConstraints {
  nLockTime: number
  nSequence: number
  csvValues: number[]
  cltvValues: number[]
  lockTimeType: 'blocks' | 'time' | 'none'
  sequenceType: 'blocks' | 'time' | 'disabled' | 'final'
  isKeyPathSpend: boolean
}

export interface ChainState {
  currentHeight: number
  currentTime: number
  commitmentHeight?: number
}

// ─── Extract ──────────────────────────────────────────────────────────────────

export function extractTimelockConstraints(node: DAGNode): TimelockConstraints {
  const tx = node.tx
  const nLockTime = tx.lockTime
  const input = tx.getInput(0)
  const nSequence = input.sequence ?? SEQUENCE_FINAL
  const isKeyPathSpend = !!input.tapKeySig

  let lockTimeType: TimelockConstraints['lockTimeType']
  if (nLockTime === 0) lockTimeType = 'none'
  else if (nLockTime < LOCKTIME_THRESHOLD) lockTimeType = 'blocks'
  else lockTimeType = 'time'

  let sequenceType: TimelockConstraints['sequenceType']
  if (nSequence === SEQUENCE_FINAL) sequenceType = 'final'
  else if (nSequence & SEQUENCE_LOCKTIME_DISABLE_FLAG) sequenceType = 'disabled'
  else if (nSequence & SEQUENCE_LOCKTIME_TYPE_FLAG) sequenceType = 'time'
  else sequenceType = 'blocks'

  const csvValues: number[] = []
  const cltvValues: number[] = []

  if (input.tapLeafScript) {
    for (const leaf of input.tapLeafScript) {
      const [_cb, scriptWithVersion] = leaf
      if (!scriptWithVersion || scriptWithVersion.length < 2) continue
      const scriptBytes = scriptWithVersion.slice(0, -1)
      try {
        const decoded = Script.decode(scriptBytes)
        extractTimelockOpcodes(decoded, csvValues, cltvValues)
      } catch {
        // Script might not be decodable — skip
      }
    }
  }

  return { nLockTime, nSequence, csvValues, cltvValues, lockTimeType, sequenceType, isKeyPathSpend }
}

function extractTimelockOpcodes(
  decoded: (string | number | Uint8Array)[],
  csvValues: number[],
  cltvValues: number[],
): void {
  for (let i = 0; i < decoded.length; i++) {
    const op = decoded[i]
    if (op === 'CHECKSEQUENCEVERIFY' && i > 0) {
      const operand = resolveScriptNumber(decoded[i - 1])
      if (operand !== null) csvValues.push(operand)
    }
    if (op === 'CHECKLOCKTIMEVERIFY' && i > 0) {
      const operand = resolveScriptNumber(decoded[i - 1])
      if (operand !== null) cltvValues.push(operand)
    }
  }
}

function resolveScriptNumber(element: string | number | Uint8Array): number | null {
  if (typeof element === 'number') return element
  if (element instanceof Uint8Array) return scriptNumToInt(element)
  return null
}

function scriptNumToInt(bytes: Uint8Array): number | null {
  if (bytes.length === 0) return 0
  if (bytes.length > 5) return null
  let result = 0
  for (let i = 0; i < bytes.length; i++) result |= bytes[i] << (8 * i)
  if (bytes[bytes.length - 1] & 0x80) {
    result &= ~(0x80 << (8 * (bytes.length - 1)))
    result = -result
  }
  if (!Number.isSafeInteger(result)) return null
  return result
}

// ─── Consistency Validation ───────────────────────────────────────────────────

export function validateTimelockConsistency(constraints: TimelockConstraints, txid: string): void {
  const { nLockTime, nSequence, csvValues, cltvValues, sequenceType, isKeyPathSpend } = constraints
  if (isKeyPathSpend) return

  if (csvValues.length > 0 && sequenceType === 'final') {
    throw new VtxoVerificationError(
      `Tx ${txid} uses OP_CSV but nSequence=0xFFFFFFFF`,
      'TIMELOCK_INCONSISTENT',
      { txid, nSequence: nSequence.toString(16), csvValues },
    )
  }

  if (cltvValues.length > 0 && nLockTime === 0) {
    const maxCltv = Math.max(...cltvValues)
    if (maxCltv > 0) {
      throw new VtxoVerificationError(
        `Tx ${txid} uses OP_CLTV (max=${maxCltv}) but nLockTime=0`,
        'TIMELOCK_INCONSISTENT',
        { txid, nLockTime, cltvValues },
      )
    }
  }

  for (const cltvVal of cltvValues) {
    if (cltvVal <= 0) continue
    const cltvIsBlocks = cltvVal < LOCKTIME_THRESHOLD
    const lockTimeIsBlocks = nLockTime < LOCKTIME_THRESHOLD
    if (nLockTime > 0 && cltvIsBlocks !== lockTimeIsBlocks) {
      throw new VtxoVerificationError(
        `Tx ${txid} CLTV/nLockTime domain mismatch`,
        'TIMELOCK_INCONSISTENT',
        { txid, cltvVal, nLockTime },
      )
    }
  }

  if (csvValues.length > 0 && sequenceType !== 'final' && sequenceType !== 'disabled') {
    const seqIsTime = !!(nSequence & SEQUENCE_LOCKTIME_TYPE_FLAG)
    const seqValue = nSequence & SEQUENCE_LOCKTIME_MASK
    for (const csvVal of csvValues) {
      if (csvVal <= 0) continue
      const csvIsTime = !!(csvVal & SEQUENCE_LOCKTIME_TYPE_FLAG)
      const csvMasked = csvVal & SEQUENCE_LOCKTIME_MASK
      if (csvIsTime !== seqIsTime) {
        throw new VtxoVerificationError(`Tx ${txid} CSV/nSequence type mismatch`, 'TIMELOCK_INCONSISTENT', {
          txid,
          csvVal,
          nSequence: nSequence.toString(16),
        })
      }
      if (csvMasked > seqValue) {
        throw new VtxoVerificationError(
          `Tx ${txid} CSV value (${csvMasked}) > nSequence value (${seqValue})`,
          'TIMELOCK_INCONSISTENT',
          { txid, csvMasked, seqValue },
        )
      }
    }
  }
}

// ─── Satisfiability Validation ────────────────────────────────────────────────

export function validateTimelockSatisfiability(
  constraints: TimelockConstraints,
  chainState: ChainState,
  txid: string,
): void {
  const { nLockTime, nSequence, cltvValues, csvValues, lockTimeType, sequenceType, isKeyPathSpend } = constraints

  if (nLockTime > 0 && lockTimeType === 'blocks' && nLockTime > chainState.currentHeight) {
    throw new VtxoVerificationError(
      `Tx ${txid} nLockTime=${nLockTime} > chain height ${chainState.currentHeight}`,
      'TIMELOCK_UNSATISFIABLE',
      { txid, nLockTime, currentHeight: chainState.currentHeight },
    )
  }

  if (nLockTime > 0 && lockTimeType === 'time' && nLockTime > chainState.currentTime) {
    throw new VtxoVerificationError(
      `Tx ${txid} nLockTime=${nLockTime} > MTP ${chainState.currentTime}`,
      'TIMELOCK_UNSATISFIABLE',
      { txid, nLockTime, currentTime: chainState.currentTime },
    )
  }

  if (!isKeyPathSpend) {
    for (const cltvVal of cltvValues) {
      if (cltvVal <= 0) continue
      if (cltvVal < LOCKTIME_THRESHOLD && cltvVal > chainState.currentHeight) {
        throw new VtxoVerificationError(
          `Tx ${txid} OP_CLTV requires block ${cltvVal} but chain is at ${chainState.currentHeight}`,
          'TIMELOCK_UNSATISFIABLE',
          { txid, cltvRequired: cltvVal, currentHeight: chainState.currentHeight },
        )
      } else if (cltvVal >= LOCKTIME_THRESHOLD && cltvVal > chainState.currentTime) {
        throw new VtxoVerificationError(
          `Tx ${txid} OP_CLTV requires time ${cltvVal} but MTP is ${chainState.currentTime}`,
          'TIMELOCK_UNSATISFIABLE',
          { txid, cltvRequired: cltvVal, currentTime: chainState.currentTime },
        )
      }
    }
  }

  if (!isKeyPathSpend && sequenceType === 'blocks' && csvValues.length > 0) {
    const maxCsv = Math.max(
      ...csvValues.filter((v) => v > 0 && !(v & SEQUENCE_LOCKTIME_TYPE_FLAG)).map((v) => v & SEQUENCE_LOCKTIME_MASK),
      0,
    )
    if (chainState.commitmentHeight !== undefined) {
      const depth = chainState.currentHeight - chainState.commitmentHeight
      if (maxCsv > depth) {
        throw new VtxoVerificationError(
          `Tx ${txid} CSV requires ${maxCsv} blocks but commitment depth is ${depth}`,
          'TIMELOCK_UNSATISFIABLE',
          { txid, csvRequired: maxCsv, currentDepth: depth },
        )
      }
    } else if (maxCsv > chainState.currentHeight) {
      throw new VtxoVerificationError(
        `Tx ${txid} CSV requires ${maxCsv} blocks which exceeds chain height ${chainState.currentHeight}`,
        'TIMELOCK_UNSATISFIABLE',
        { txid, csvRequired: maxCsv, currentHeight: chainState.currentHeight },
      )
    }
  }
}

// ─── Entry Point ──────────────────────────────────────────────────────────────

export function verifyDAGTimelocks(rootNode: DAGNode, chainState: ChainState): void {
  const stack: DAGNode[] = [rootNode]
  while (stack.length > 0) {
    const node = stack.pop()!
    const constraints = extractTimelockConstraints(node)
    const hasTimelocks =
      constraints.nLockTime > 0 ||
      constraints.sequenceType !== 'final' ||
      constraints.csvValues.length > 0 ||
      constraints.cltvValues.length > 0
    if (hasTimelocks) {
      validateTimelockConsistency(constraints, node.txid)
      validateTimelockSatisfiability(constraints, chainState, node.txid)
    }
    for (const child of node.children.values()) stack.push(child)
  }
}
