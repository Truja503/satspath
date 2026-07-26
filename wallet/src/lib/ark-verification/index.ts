/**
 * Ark VTXO Client-Side Verification — Public API
 *
 * Zero-trust verification of Ark Virtual Transaction Outputs (VTXOs).
 * Adapted from ARK/src/ for use in Arkade Money wallet (browser).
 */

export {
  // Main pipeline
  verifyVtxoComplete,
  reconstructAndValidateVtxoDAG,
  onReceiveVtxo,
  verifyOnchainAnchoring,
  // Types
  ChainTxType,
  VtxoVerificationError,
  BATCH_OUTPUT_VTXO_INDEX,
} from './vtxoDAGVerification'

export type {
  Outpoint,
  ChainTx,
  VtxoChain,
  IndexerProvider,
  OnchainProvider,
  StorageProvider,
  DAGNode,
  DAGValidationResult,
  CheckpointValidation,
} from './vtxoDAGVerification'

export type { ChainState, TimelockConstraints } from './timelockVerification'
export type { HashCondition, PreimageVerificationResult } from './hashPreimageVerification'
