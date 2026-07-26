/**
 * ============================================================================
 *  VTXO DAG Verification — Client-side verification of VTXO chains for Ark
 * ============================================================================
 *
 *  Adapted from /home/chelo/antigravity/PlanB/ARK/src/vtxoDAGVerification.ts
 *  Changes for wallet integration (v2 deps, browser-compatible, no Buffer):
 *    - Updated @scure/btc-signer to v2 import paths
 *    - Updated @noble/hashes import to v2
 *    - Removed Buffer usage — pure Uint8Array
 *    - All internal cross-imports use relative paths
 *
 *  ZERO TRUST: Every piece of data from the ASP is treated as potentially
 *  malicious. The function fails loudly on any inconsistency.
 * ============================================================================
 */

import { Transaction } from '@scure/btc-signer'
import { hex, base64 } from '@scure/base'
import { sha256 } from '@noble/hashes/sha2.js'
import { verifyDAGSignatures } from './signatureVerification'
import { verifyNodeTaproot } from './taprootVerification'
import { verifyDAGTimelocks, type ChainState } from './timelockVerification'
import { verifyDAGHashPreimages } from './hashPreimageVerification'
import { ConcurrencyLimiter, VerificationCache } from './performanceUtils'

// ─── Performance Buffers ─────────────────────────────────────────────────────
const globalVerificationCache = new VerificationCache()
const globalOnchainLimiter = new ConcurrencyLimiter(10)

// ─── Helpers ─────────────────────────────────────────────────────────────────

/**
 * Compute txid manually: REVERSE(SHA256d(non-witness serialization)).
 * btc-signer v2 still throws on unsigned PSBTs for .id — compute manually.
 */
function computeTxid(tx: Transaction): string {
  const rawBytes = tx.toBytes(true, false)
  const hash1 = sha256(rawBytes)
  const hash2 = sha256(hash1)
  const reversed = new Uint8Array(hash2)
  reversed.reverse()
  return hex.encode(reversed)
}

// ─── Types ───────────────────────────────────────────────────────────────────

export interface Outpoint {
  txid: string
  vout: number
}

export enum ChainTxType {
  UNSPECIFIED = 'INDEXER_CHAINED_TX_TYPE_UNSPECIFIED',
  COMMITMENT = 'INDEXER_CHAINED_TX_TYPE_COMMITMENT',
  ARK = 'INDEXER_CHAINED_TX_TYPE_ARK',
  TREE = 'INDEXER_CHAINED_TX_TYPE_TREE',
  CHECKPOINT = 'INDEXER_CHAINED_TX_TYPE_CHECKPOINT',
}

export interface ChainTx {
  txid: string
  expiresAt: string
  type: ChainTxType
  spends: string[]
}

export interface VtxoChain {
  chain: ChainTx[]
}

export interface IndexerProvider {
  getBatchVtxos(commitmentTxid: string): Promise<VtxoChain[]>
  getVtxoChain?(txid: string, vout: number): Promise<VtxoChain>
  getVirtualTxs(txids: string[]): Promise<{ txs: string[] }>
}

export interface OnchainProvider {
  getRawTransaction(txid: string): Promise<string>
  getTxStatus(txid: string): Promise<{
    confirmed: boolean
    blockHeight?: number
    blockTime?: number
  }>
  getBlockchainInfo?(): Promise<{ height: number; medianTime: number }>
  broadcastTransaction(txHex: string): Promise<string>
}

export interface StorageProvider {
  setItem(key: string, value: string): Promise<void>
  getItem(key: string): Promise<string | null>
  removeItem(key: string): Promise<void>
}

export interface DAGNode {
  txid: string
  tx: Transaction
  chainTx: ChainTx
  rawPsbt: string
  children: Map<number, DAGNode>
  descendant: DAGNode | null
  ancestor: DAGNode | null
  ancestorOutputIndex: number | null
}

export interface DAGValidationResult {
  valid: boolean
  vtxoRoot: DAGNode
  anchoringLeaf: DAGNode
  commitmentTxid: string
  batchOutputIndex: number
  checkpointValidations: CheckpointValidation[]
  diagnostics: string[]
}

export interface CheckpointValidation {
  txid: string
  expiryCoherent: boolean
  parentChainValid: boolean
  notes: string[]
}

// ─── Errors ──────────────────────────────────────────────────────────────────

export class VtxoVerificationError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly context?: Record<string, unknown>,
  ) {
    super(`[VTXO-VERIFY:${code}] ${message}`)
    this.name = 'VtxoVerificationError'
  }
}

const Errors = {
  EMPTY_CHAIN: (vtxo: Outpoint) =>
    new VtxoVerificationError(`Empty chain for VTXO ${vtxo.txid}:${vtxo.vout}`, 'EMPTY_CHAIN', { vtxo }),

  NO_COMMITMENT: () =>
    new VtxoVerificationError('No commitment tx at anchoring leaf', 'NO_COMMITMENT'),

  MISSING_TX: (txid: string) =>
    new VtxoVerificationError(`Virtual tx ${txid} not returned by ASP`, 'MISSING_TX', { txid }),

  TXID_MISMATCH: (expected: string, actual: string) =>
    new VtxoVerificationError(
      `Txid mismatch: ASP claims ${expected} but PSBT computes to ${actual}`,
      'TXID_MISMATCH',
      { expected, actual },
    ),

  INPUT_CHAIN_BREAK: (childTxid: string, expectedParent: string, actualParent: string) =>
    new VtxoVerificationError(
      `Chain break: ${childTxid} should reference ${expectedParent} but references ${actualParent}`,
      'INPUT_CHAIN_BREAK',
      { childTxid, expectedParent, actualParent },
    ),

  AMOUNT_MISMATCH: (parentTxid: string, outputIndex: number, parentAmount: bigint, childSum: bigint) =>
    new VtxoVerificationError(
      `Amount mismatch: ${parentTxid}[${outputIndex}] = ${parentAmount} sats, child sum = ${childSum}`,
      'AMOUNT_MISMATCH',
      { parentTxid, outputIndex, parentAmount: parentAmount.toString(), childSum: childSum.toString() },
    ),

  CHECKPOINT_EXPIRY_INCOHERENT: (txid: string, details: string) =>
    new VtxoVerificationError(`Checkpoint ${txid} incoherent expiry: ${details}`, 'CHECKPOINT_EXPIRY_INCOHERENT', {
      txid,
    }),

  ORPHAN_TX: (txid: string) =>
    new VtxoVerificationError(`Tx ${txid} is orphaned — not reachable from VTXO root`, 'ORPHAN_TX', { txid }),
} as const

export const BATCH_OUTPUT_VTXO_INDEX = 0

// ─── Main Public Function ─────────────────────────────────────────────────────

export async function reconstructAndValidateVtxoDAG(
  vtxoRootOutpoint: Outpoint,
  indexer: IndexerProvider,
  onchain: OnchainProvider,
  witnessPreimages?: Map<string, Uint8Array>,
): Promise<DAGValidationResult> {
  const diagnostics: string[] = []

  // Step 1: Fetch the VTXO chain
  diagnostics.push(`[1/6] Fetching VTXO chain for ${vtxoRootOutpoint.txid}:${vtxoRootOutpoint.vout}`)
  let chain: ChainTx[]

  if (indexer.getVtxoChain) {
    const vtxoChain = await indexer.getVtxoChain(vtxoRootOutpoint.txid, vtxoRootOutpoint.vout)
    if (!vtxoChain || vtxoChain.chain.length === 0) throw Errors.EMPTY_CHAIN(vtxoRootOutpoint)
    chain = vtxoChain.chain
    diagnostics.push(`  → Direct mode: ${chain.length} links`)
  } else {
    const allChains = await indexer.getBatchVtxos(vtxoRootOutpoint.txid)
    const vtxoChain = allChains.find((vc) => vc.chain.some((link) => link.txid === vtxoRootOutpoint.txid))
    if (!vtxoChain || vtxoChain.chain.length === 0) throw Errors.EMPTY_CHAIN(vtxoRootOutpoint)
    chain = vtxoChain.chain
    diagnostics.push(`  → Privacy mode: ${chain.length} links`)
  }

  // Step 2: Separate commitment from virtual transactions
  const commitmentLinks = chain.filter((c) => c.type === ChainTxType.COMMITMENT)
  const virtualLinks = chain.filter((c) => c.type !== ChainTxType.COMMITMENT && c.type !== ChainTxType.UNSPECIFIED)

  if (commitmentLinks.length === 0) throw Errors.NO_COMMITMENT()
  const actualCommitmentTxid = commitmentLinks[0].txid
  diagnostics.push(`[2/6] Commitment tx: ${actualCommitmentTxid}`)

  // Step 3: Fetch all virtual transaction PSBTs
  diagnostics.push(`[3/6] Fetching ${virtualLinks.length} virtual tx PSBTs`)
  const virtualTxids = virtualLinks.map((l) => l.txid)
  const rawPsbts = await fetchAllVirtualTxs(indexer, virtualTxids)

  const txMap = new Map<string, { tx: Transaction; rawPsbt: string; chainTx: ChainTx }>()
  for (const link of virtualLinks) {
    const rawPsbt = rawPsbts.get(link.txid)
    if (!rawPsbt) throw Errors.MISSING_TX(link.txid)

    let tx: Transaction
    try {
      tx = Transaction.fromPSBT(base64.decode(rawPsbt), { allowUnknownOutputs: true })
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e)
      throw new VtxoVerificationError(`Failed to parse PSBT for ${link.txid}: ${msg}`, 'INVALID_PSBT', {
        txid: link.txid,
      })
    }

    const computed = computeTxid(tx)
    if (computed !== link.txid) throw Errors.TXID_MISMATCH(link.txid, computed)
    txMap.set(link.txid, { tx, rawPsbt, chainTx: link })
  }

  // Step 4: Reconstruct the DAG
  diagnostics.push('[4/6] Reconstructing DAG structure')
  const chainLookup = new Map<string, ChainTx>()
  for (const link of chain) chainLookup.set(link.txid, link)

  let anchoringLeaf: DAGNode | null = null
  const allNodes = new Map<string, DAGNode>()

  for (const [txid, { tx, rawPsbt, chainTx }] of txMap) {
    allNodes.set(txid, {
      txid,
      tx,
      chainTx,
      rawPsbt,
      children: new Map(),
      ancestor: null,
      ancestorOutputIndex: null,
      descendant: null,
    })
  }

  // Wire relationships with cycle detection
  for (const node of allNodes.values()) {
    const pathVisited = new Set<string>()
    let tracer: DAGNode | null = node
    while (tracer) {
      if (pathVisited.has(tracer.txid)) {
        throw new VtxoVerificationError(`Cycle detected at ${tracer.txid}`, 'CYCLE_DETECTED')
      }
      pathVisited.add(tracer.txid)
      const input = tracer.tx.getInput(0)
      if (!input.txid) break
      const pTxid = hex.encode(input.txid)
      if (pTxid === actualCommitmentTxid) break
      tracer = allNodes.get(pTxid) ?? null
    }

    const input = node.tx.getInput(0)
    const ancestorTxid = hex.encode(input.txid!)
    const ancestorOutputIndex = input.index ?? 0

    if (ancestorTxid === actualCommitmentTxid) {
      node.ancestor = null
      node.ancestorOutputIndex = ancestorOutputIndex
      anchoringLeaf = node
    } else {
      const ancestorNode = allNodes.get(ancestorTxid)
      if (!ancestorNode) throw Errors.INPUT_CHAIN_BREAK(node.txid, ancestorTxid, '(not in DAG)')
      node.ancestor = ancestorNode
      node.ancestorOutputIndex = ancestorOutputIndex
      ancestorNode.children.set(ancestorOutputIndex, node)
    }
  }

  if (!anchoringLeaf) throw Errors.NO_COMMITMENT()

  // Orphan check
  const reachable = new Set<string>()
  collectReachable(anchoringLeaf, reachable)
  for (const txid of allNodes.keys()) {
    if (!reachable.has(txid)) throw Errors.ORPHAN_TX(txid)
  }

  const vtxoRoot = allNodes.get(vtxoRootOutpoint.txid) ?? null
  if (!vtxoRoot) {
    throw new VtxoVerificationError(`VTXO Root ${vtxoRootOutpoint.txid} not found in chain`, 'ROOT_NOT_FOUND')
  }

  // Step 5: Fetch on-chain anchoring context
  diagnostics.push('[5/6] Fetching on-chain anchoring status')
  const commitmentRaw = await onchain.getRawTransaction(actualCommitmentTxid)
  const commitmentTx = Transaction.fromRaw(hex.decode(commitmentRaw), { allowUnknownOutputs: true })
  const batchOutput = commitmentTx.getOutput(anchoringLeaf.ancestorOutputIndex ?? BATCH_OUTPUT_VTXO_INDEX)
  ;(anchoringLeaf as DAGNode & { prevOutContext?: unknown }).prevOutContext = {
    script: batchOutput.script,
    amount: batchOutput.amount,
  }

  const onchainStatus = await onchain.getTxStatus(actualCommitmentTxid)
  const blockchainInfo = onchain.getBlockchainInfo ? await onchain.getBlockchainInfo() : null

  // Steps 6: Validations
  diagnostics.push('[6/6] Running validations')
  validateDAGChaining(anchoringLeaf, actualCommitmentTxid, diagnostics)
  const checkpointValidations = validateCheckpoints(allNodes, chainLookup, actualCommitmentTxid, diagnostics)

  for (const node of allNodes.values()) verifyNodeTaproot(node)
  verifyDAGSignatures(anchoringLeaf)

  if (blockchainInfo) {
    const chainState: ChainState = {
      currentHeight: blockchainInfo.height,
      currentTime: blockchainInfo.medianTime,
      commitmentHeight: onchainStatus.confirmed ? onchainStatus.blockHeight : undefined,
    }
    verifyDAGTimelocks(anchoringLeaf, chainState)
  }

  verifyDAGHashPreimages(anchoringLeaf, witnessPreimages)

  return {
    valid: true,
    vtxoRoot,
    anchoringLeaf,
    commitmentTxid: actualCommitmentTxid,
    batchOutputIndex: anchoringLeaf.ancestorOutputIndex ?? BATCH_OUTPUT_VTXO_INDEX,
    checkpointValidations,
    diagnostics,
  }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

async function fetchAllVirtualTxs(indexer: IndexerProvider, txids: string[]): Promise<Map<string, string>> {
  const result = new Map<string, string>()
  const BATCH_SIZE = 50
  for (let i = 0; i < txids.length; i += BATCH_SIZE) {
    const batch = txids.slice(i, i + BATCH_SIZE)
    const { txs } = await indexer.getVirtualTxs(batch)
    for (let j = 0; j < batch.length; j++) {
      if (j < txs.length && txs[j]) result.set(batch[j], txs[j])
    }
  }
  return result
}

function collectReachable(node: DAGNode, reachable: Set<string>): void {
  const stack: DAGNode[] = [node]
  while (stack.length > 0) {
    const current = stack.pop()!
    reachable.add(current.txid)
    for (const child of current.children.values()) stack.push(child)
  }
}

function validateDAGChaining(rootNode: DAGNode, commitmentTxid: string, diagnostics: string[]): void {
  const stack: DAGNode[] = [rootNode]
  while (stack.length > 0) {
    const node = stack.pop()!

    if (node.ancestor === null) {
      const input = node.tx.getInput(0)
      if (!input.txid) throw Errors.INPUT_CHAIN_BREAK(node.txid, commitmentTxid, '(no input)')
      const inputTxid = hex.encode(input.txid)
      if (inputTxid !== commitmentTxid) throw Errors.INPUT_CHAIN_BREAK(node.txid, commitmentTxid, inputTxid)

      diagnostics.push(`  ✓ Anchoring Leaf ${node.txid} anchored to commitment ${commitmentTxid}`)

      const anchorPrevOut = (node as DAGNode & { prevOutContext?: { script?: Uint8Array; amount?: bigint } })
        .prevOutContext
      if (anchorPrevOut?.amount !== undefined) {
        let anchorOutputsSum = BigInt(0)
        for (let i = 0; i < node.tx.outputsLength; i++) {
          const out = node.tx.getOutput(i)
          if (out?.amount) anchorOutputsSum += out.amount
        }
        if (anchorOutputsSum !== anchorPrevOut.amount) {
          throw Errors.AMOUNT_MISMATCH(commitmentTxid, input.index ?? 0, anchorPrevOut.amount, anchorOutputsSum)
        }
        diagnostics.push(`  ✓ Amount ${anchorOutputsSum} sats conserved`)
      }
    }

    for (const [outputIndex, child] of node.children) {
      const childInput = child.tx.getInput(0)
      if (!childInput.txid) throw Errors.INPUT_CHAIN_BREAK(child.txid, node.txid, '(no input txid)')

      const childInputTxid = hex.encode(childInput.txid)
      const childInputIndex = childInput.index ?? 0

      if (childInputTxid !== node.txid) throw Errors.INPUT_CHAIN_BREAK(child.txid, node.txid, childInputTxid)
      if (childInputIndex !== outputIndex) {
        throw new VtxoVerificationError(
          `Child ${child.txid} input index ${childInputIndex} ≠ expected ${outputIndex}`,
          'INDEX_MISMATCH',
          { childTxid: child.txid, expected: outputIndex, actual: childInputIndex },
        )
      }

      const ancestorOutput = node.tx.getOutput(outputIndex)
      if (!ancestorOutput || ancestorOutput.amount === undefined) {
        throw new VtxoVerificationError(`Ancestor ${node.txid} has no output at ${outputIndex}`, 'MISSING_OUTPUT', {
          ancestorTxid: node.txid,
          outputIndex,
        })
      }

      let childOutputsSum = BigInt(0)
      for (let i = 0; i < child.tx.outputsLength; i++) {
        const out = child.tx.getOutput(i)
        if (out?.amount) childOutputsSum += out.amount
      }
      if (childOutputsSum !== ancestorOutput.amount) {
        throw Errors.AMOUNT_MISMATCH(node.txid, outputIndex, ancestorOutput.amount, childOutputsSum)
      }

      diagnostics.push(`  ✓ ${child.txid} → ${node.txid}[${outputIndex}]: ${ancestorOutput.amount} sats OK`)
      stack.push(child)
    }
  }
}

function validateCheckpoints(
  allNodes: Map<string, DAGNode>,
  chainLookup: Map<string, ChainTx>,
  commitmentTxid: string,
  diagnostics: string[],
): CheckpointValidation[] {
  const results: CheckpointValidation[] = []

  for (const [txid, node] of allNodes) {
    if (node.chainTx.type !== ChainTxType.CHECKPOINT) continue

    const notes: string[] = []
    let expiryCoherent = true
    let parentChainValid = true

    if (node.chainTx.spends.length === 0) {
      notes.push('WARNING: Checkpoint has no ancestor references')
      parentChainValid = false
    }

    const input = node.tx.getInput(0)
    if (!input.txid) {
      notes.push('ERROR: Checkpoint has no input txid')
      parentChainValid = false
    } else {
      const ancestorTxid = hex.encode(input.txid)
      const ancestorInChain = chainLookup.get(ancestorTxid)
      if (!ancestorInChain) {
        notes.push(`WARNING: Checkpoint ancestor ${ancestorTxid} not in chain`)
      } else {
        notes.push(`Ancestor: ${ancestorTxid} (${ancestorInChain.type})`)
      }
    }

    const checkpointExpiry = parseExpiry(node.chainTx.expiresAt)

    if (node.ancestor) {
      const ancestorExpiry = parseExpiry(node.ancestor.chainTx.expiresAt)
      if (checkpointExpiry > 0 && ancestorExpiry > 0) {
        if (checkpointExpiry < ancestorExpiry) {
          expiryCoherent = false
          throw Errors.CHECKPOINT_EXPIRY_INCOHERENT(txid, `expires ${checkpointExpiry} but ancestor at ${ancestorExpiry}`)
        }
        notes.push(`Expiry OK: checkpoint=${checkpointExpiry}, ancestor=${ancestorExpiry}`)
      }
    }

    const commitmentChainTx = chainLookup.get(commitmentTxid)
    if (commitmentChainTx) {
      const batchRootExpiry = parseExpiry(commitmentChainTx.expiresAt)
      if (checkpointExpiry > 0 && batchRootExpiry > 0 && checkpointExpiry > batchRootExpiry) {
        expiryCoherent = false
        throw Errors.CHECKPOINT_EXPIRY_INCOHERENT(txid, `expires ${checkpointExpiry} but batch at ${batchRootExpiry}`)
      }
    }

    if (input.txid) {
      const sequence = input.sequence
      if (sequence !== undefined && sequence !== 0xffffffff) {
        const isTimeBased = (sequence & (1 << 22)) !== 0
        const timelockValue = sequence & 0xffff
        notes.push(`Sweep delay: ${timelockValue} ${isTimeBased ? 'seconds (×512)' : 'blocks'}`)
      }
    }

    diagnostics.push(`  ${expiryCoherent && parentChainValid ? '✓' : '✗'} Checkpoint ${txid}: ${notes.join('; ')}`)
    results.push({ txid, expiryCoherent, parentChainValid, notes })
  }

  if (results.length === 0) diagnostics.push('  (no checkpoint transactions in this chain)')
  return results
}

function parseExpiry(expiresAt: string): number {
  if (!expiresAt) return 0
  const n = Number(expiresAt)
  if (Number.isFinite(n) && n > 0) return n < 1e12 ? n * 1000 : n
  const d = new Date(expiresAt)
  return isNaN(d.getTime()) ? 0 : d.getTime()
}

// ─── On-chain Anchoring Verification ─────────────────────────────────────────

export async function verifyOnchainAnchoring(
  commitmentTxid: string,
  outputIndex: number,
  expectedAmount: bigint,
  expectedScript: Uint8Array,
  onchain: OnchainProvider,
  minConfirmations = 1,
): Promise<{ confirmed: boolean; blockHeight?: number; blockTime?: number }> {
  const status = await onchain.getTxStatus(commitmentTxid)
  if (!status.confirmed) {
    throw new VtxoVerificationError(`Commitment tx ${commitmentTxid} is not confirmed`, 'COMMITMENT_NOT_CONFIRMED', {
      commitmentTxid,
    })
  }

  if (status.blockHeight !== undefined && onchain.getBlockchainInfo) {
    const info = await onchain.getBlockchainInfo()
    const confirmations = info.height - status.blockHeight + 1
    if (confirmations < minConfirmations) {
      throw new VtxoVerificationError(
        `Insufficient confirmations (${confirmations} < ${minConfirmations})`,
        'INSUFFICIENT_CONFIRMATIONS',
        { commitmentTxid, confirmations, required: minConfirmations },
      )
    }
  }

  const rawHex = await onchain.getRawTransaction(commitmentTxid)
  const tx = Transaction.fromRaw(hex.decode(rawHex), { allowUnknownOutputs: true })

  if (outputIndex >= tx.outputsLength) {
    throw new VtxoVerificationError(`No output at index ${outputIndex}`, 'ANCHOR_OUTPUT_NOT_FOUND', {
      commitmentTxid,
      outputIndex,
    })
  }

  const actualOutput = tx.getOutput(outputIndex)
  if (actualOutput.amount === undefined || actualOutput.script === undefined) {
    throw new VtxoVerificationError('Commitment output malformed', 'MALFORMED_ANCHOR_OUTPUT', {
      commitmentTxid,
      outputIndex,
    })
  }

  if (actualOutput.amount !== expectedAmount) {
    throw new VtxoVerificationError(
      `Amount mismatch: expected ${expectedAmount}, got ${actualOutput.amount}`,
      'ANCHOR_AMOUNT_MISMATCH',
      { commitmentTxid, outputIndex },
    )
  }

  if (hex.encode(actualOutput.script) !== hex.encode(expectedScript)) {
    throw new VtxoVerificationError('Script mismatch', 'ANCHOR_SCRIPT_MISMATCH', { commitmentTxid, outputIndex })
  }

  return status
}

// ─── Full Pipeline ────────────────────────────────────────────────────────────

export async function verifyVtxoComplete(
  vtxoOutpoint: Outpoint,
  indexer: IndexerProvider,
  onchain: OnchainProvider,
  minConfirmations = 1,
  witnessPreimages?: Map<string, Uint8Array>,
): Promise<DAGValidationResult & { onchainStatus: { confirmed: boolean; blockHeight?: number } }> {
  const cacheKey = `${vtxoOutpoint.txid}:${vtxoOutpoint.vout}:${minConfirmations}`
  const cached = globalVerificationCache.get(cacheKey)
  if (cached) return cached as Awaited<ReturnType<typeof verifyVtxoComplete>>

  const dagResult = await reconstructAndValidateVtxoDAG(vtxoOutpoint, indexer, onchain, witnessPreimages)

  const onchainStatus = await globalOnchainLimiter.run(async () => {
    const anchor = (dagResult.anchoringLeaf as DAGNode & { prevOutContext?: { amount?: bigint; script?: Uint8Array } })
      .prevOutContext
    if (!anchor?.amount || !anchor?.script) {
      return onchain.getTxStatus(dagResult.commitmentTxid).then((status) => {
        if (!status.confirmed)
          throw new VtxoVerificationError('Commitment not confirmed', 'COMMITMENT_NOT_CONFIRMED')
        return status
      })
    }
    return verifyOnchainAnchoring(
      dagResult.commitmentTxid,
      dagResult.batchOutputIndex,
      anchor.amount,
      anchor.script,
      onchain,
      minConfirmations,
    )
  })

  dagResult.diagnostics.push(`✓ Commitment ${dagResult.commitmentTxid} confirmed at block ${onchainStatus.blockHeight}`)

  const finalResult = { ...dagResult, onchainStatus }
  globalVerificationCache.set(cacheKey, finalResult)
  return finalResult
}

// ─── Convenience: onReceiveVtxo ──────────────────────────────────────────────

/**
 * Called when a new VTXO is detected. Runs the full verification pipeline
 * and returns a structured result — never throws.
 */
export async function onReceiveVtxo(
  outpoint: Outpoint,
  indexer: IndexerProvider,
  onchain: OnchainProvider,
): Promise<{ success: boolean; diagnostics: string[]; error?: string }> {
  try {
    const result = await reconstructAndValidateVtxoDAG(outpoint, indexer, onchain)
    return {
      success: true,
      diagnostics: [...result.diagnostics, `✓ VTXO ${outpoint.txid}:${outpoint.vout} verified (zero-trust)`],
    }
  } catch (error: unknown) {
    const msg = error instanceof Error ? error.message : String(error)
    return { success: false, diagnostics: ['Verification pipeline terminated'], error: msg }
  }
}
