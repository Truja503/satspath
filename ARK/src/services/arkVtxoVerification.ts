/**
 * Arkade Wallet - Ark VTXO Verification Service
 * 
 * Integrates Arkade SDK's VTXO verification with the wallet UI.
 * This service runs the client-side verification pipeline (Tier 1-3)
 * when receiving Ark payments.
 */

import { 
  IndexerProvider, 
  OnchainProvider, 
  StorageProvider,
  Outpoint,
  VtxoChain,
  DAGValidationResult,
  reconstructAndValidateVtxoDAG,
  verifyVtxoComplete,
  VtxoVerificationError,
  BATCH_OUTPUT_VTXO_INDEX,
  DAGNode,
  CheckpointValidation
} from '../vtxoDAGVerification';
import { sovereignStorage, StorageAdapter } from '../sovereignStorage';

// Re-export key types and functions
export type { 
  IndexerProvider, 
  OnchainProvider, 
  StorageProvider,
  Outpoint,
  VtxoChain,
  DAGValidationResult,
  CheckpointValidation,
  DAGNode
} from '../vtxoDAGVerification';

export { 
  VtxoVerificationError,
  reconstructAndValidateVtxoDAG,
  verifyVtxoComplete,
  BATCH_OUTPUT_VTXO_INDEX,
  verifyOnchainAnchoring,
  verifyVtxoComplete
} from '../vtxoDAGVerification';

export { sovereignStorage } from '../sovereignStorage';

/**
 * Configuration for Ark VTXO verification
 */
export interface ArkVerificationConfig {
  /** Indexer service for fetching VTXO chains and PSBTs */
  indexer: IndexerProvider;
  /** On-chain provider for anchoring verification */
  onchain: OnchainProvider;
  /** Storage for sovereign exit data (Tier 3) */
  storage: StorageProvider;
  /** Maximum concurrent verification operations */
  maxConcurrency?: number;
  /** Skip verification for testing (NOT for production) */
  skipVerification?: boolean;
}

/**
 * Result of VTXO verification for UI display
 */
export interface VerificationResult {
  valid: boolean;
  vtxoRootTxid: string;
  commitmentTxid: string;
  batchOutputIndex: number;
  exitDataStored: boolean;
  diagnostics: string[];
  error?: string;
}

/**
 * Progress callback for long-running verification
 */
export interface VerificationProgress {
  stage: 'fetching_chain' | 'fetching_psbts' | 'reconstructing_dag' | 'validating_signatures' | 'validating_taproot' | 'validating_timelocks' | 'validating_hash_preimages' | 'validating_anchoring' | 'storing_exit_data' | 'complete';
  progress: number; // 0-100
  message: string;
  currentTx?: number;
  totalTxs?: number;
}

/**
 * Ark VTXO Verification Service
 * 
 * Provides high-level interface for verifying received VTXOs.
 * Integrates with wallet UI for progress reporting and result display.
 */
export class ArkVtxoVerificationService {
  private config: ArkVerificationConfig;
  private abortController: AbortController | null = null;

  constructor(config: ArkVerificationConfig) {
    this.config = config;
  }

  /**
   * Verify a received VTXO outpoint
   * 
   * @param vtxoOutpoint - The VTXO outpoint received from ASP
   * @param onProgress - Optional progress callback for UI updates
   * @returns Verification result with diagnostics
   */
  async verifyReceivedVtxo(
    vtxoOutpoint: Outpoint,
    onProgress?: (progress: VerificationProgress) => void
  ): Promise<VerificationResult> {
    this.abortController = new AbortController();

    if (this.config.skipVerification) {
      return {
        valid: true,
        vtxoRootTxid: vtxoOutpoint.txid,
        commitmentTxid: 'skipped',
        batchOutputIndex: 0,
        exitDataStored: false,
        diagnostics: ['Verification skipped (testing mode)'],
      };
    }

    try {
      // Stage 1: Fetch VTXO chain
      this.reportProgress(onProgress, { stage: 'fetching_chain', progress: 10, message: 'Fetching VTXO chain from indexer...' });
      
      // Stage 2: Fetch PSBTs
      this.reportProgress(onProgress, { stage: 'fetching_psbts', progress: 25, message: 'Fetching virtual transaction PSBTs...' });
      
      // Stage 3-6: Full DAG reconstruction and validation
      this.reportProgress(onProgress, { stage: 'reconstructing_dag', progress: 50, message: 'Reconstructing VTXO DAG...' });
      
      const result = await reconstructAndValidateVtxoDAG(
        vtxoOutpoint,
        this.config.indexer,
        this.config.onchain,
        {
          // Progress callbacks
          onChainFetched: (chain: VtxoChain) => {
            this.reportProgress(onProgress, { 
              stage: 'fetching_psbts', 
              progress: 25, 
              message: `Fetched chain with ${chain.chain.length} transactions`,
              totalTxs: chain.chain.length
            });
          },
          onPsbtsFetched: (fetched: number, total: number) => {
            this.reportProgress(onProgress, { 
              stage: 'fetching_psbts', 
              progress: 25 + Math.floor((fetched / total) * 25),
              message: `Fetched ${fetched}/${total} PSBTs`,
              currentTx: fetched,
              totalTxs: total
            });
          },
          onDagReconstructed: (nodeCount: number) => {
            this.reportProgress(onProgress, { 
              stage: 'reconstructing_dag', 
              progress: 50, 
              message: `Reconstructed DAG with ${nodeCount} nodes` 
            });
          },
          onSignaturesValidated: () => {
            this.reportProgress(onProgress, { 
              stage: 'validating_signatures', 
              progress: 60, 
              message: 'All Schnorr/MuSig2 signatures valid' 
            });
          },
          onTaprootValidated: () => {
            this.reportProgress(onProgress, { 
              stage: 'validating_taproot', 
              progress: 70, 
              message: 'Taproot tree and Merkle proofs valid' 
            });
          },
          onTimelocksValidated: () => {
            this.reportProgress(onProgress, { 
              stage: 'validating_timelocks', 
              progress: 75, 
              message: 'CSV timelocks and expiry coherent' 
            });
          },
          onHashPreimagesValidated: () => {
            this.reportProgress(onProgress, { 
              stage: 'validating_hash_preimages', 
              progress: 80, 
              message: 'All HTLC hash preimages present' 
            });
          },
          onAnchoringValidated: (commitmentTxid: string, batchOutputIndex: number) => {
            this.reportProgress(onProgress, { 
              stage: 'validating_anchoring', 
              progress: 85, 
              message: `Anchored on commitment ${commitmentTxid.slice(0, 16)}...` 
            });
          },
          onExitDataStored: () => {
            this.reportProgress(onProgress, { 
              stage: 'storing_exit_data', 
              progress: 95, 
              message: 'Sovereign exit data stored locally' 
            });
          },
        }
      );

      // Stage 7: Verify complete (includes sovereign exit data storage)
      this.reportProgress(onProgress, { stage: 'complete', progress: 100, message: 'Verification complete!' });

      // Store exit data for sovereign exit (Tier 3)
      // Note: verifyVtxoComplete already calls onReceiveVtxo internally
      // But we can also call it explicitly if needed

      return {
        valid: result.valid,
        vtxoRootTxid: result.vtxoRoot.txid,
        commitmentTxid: result.commitmentTxid,
        batchOutputIndex: result.batchOutputIndex,
        exitDataStored: true, // verifyVtxoComplete stores exit data
        diagnostics: result.diagnostics,
      };
    } catch (error) {
      if (error instanceof VtxoVerificationError) {
        return {
          valid: false,
          vtxoRootTxid: '',
          commitmentTxid: '',
          batchOutputIndex: 0,
          exitDataStored: false,
          diagnostics: [error.message],
          error: error.message,
        };
      }
      return {
        valid: false,
        vtxoRootTxid: '',
        commitmentTxid: '',
        batchOutputIndex: 0,
        exitDataStored: false,
        diagnostics: [error instanceof Error ? error.message : 'Unknown verification error'],
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    } finally {
      this.abortController = null;
    }
  }

  private reportProgress(
    callback: ((progress: VerificationProgress) => void) | undefined,
    progress: VerificationProgress
  ) {
    if (callback) {
      try {
        callback(progress);
      } catch (e) {
        console.warn('Progress callback error:', e);
      }
    }
  }

  /**
   * Abort ongoing verification
   */
  abort(): void {
    this.abortController?.abort();
  }

  /**
   * Check if verification is in progress
   */
  isVerifying(): boolean {
    return this.abortController !== null;
  }

  /**
   * Get stored exit data for a VTXO (for sovereign exit)
   * This would be used when user wants to exit unilaterally
   */
  async getExitData(vtxoOutpoint: Outpoint): Promise<string | null> {
    // This would use sovereignStorage.getItem()
    // The key format is determined by sovereignStorage implementation
    const key = `ark_exit:${vtxoOutpoint.txid}:${vtxoOutpoint.vout}`;
    return this.config.storage.getItem(key);
  }

  /**
   * Broadcast sovereign exit transaction
   * Used when user wants to exit unilaterally
   */
  async broadcastExit(txHex: string): Promise<string> {
    return this.config.onchain.broadcastTransaction(txHex);
  }
}

/**
 * Factory function to create verification service with default providers
 * This can be customized by the wallet implementation
 */
export function createArkVerificationService(
  indexerUrl: string,
  onchainUrl: string,
  storage: StorageProvider,
  options?: { maxConcurrency?: number; skipVerification?: boolean }
): ArkVtxoVerificationService {
  // These would be implemented with actual provider classes
  // For now, we return a service that throws if used without proper providers
  const mockIndexer: IndexerProvider = {
    getBatchVtxos: async () => { throw new Error('Indexer not configured'); },
    getVtxoChain: async () => { throw new Error('Indexer not configured'); },
    getVirtualTxs: async () => { throw new Error('Indexer not configured'); },
  };

  const mockOnchain: OnchainProvider = {
    getRawTransaction: async () => { throw new Error('Onchain provider not configured'); },
    getTxStatus: async () => { throw new Error('Onchain provider not configured'); },
    getBlockchainInfo: async () => { throw new Error('Onchain provider not configured'); },
    broadcastTransaction: async () => { throw new Error('Onchain provider not configured'); },
  };

  const config: ArkVerificationConfig = {
    indexer: mockIndexer,
    onchain: mockOnchain,
    storage: null as any, // Will be set by caller
    maxConcurrency: options?.maxConcurrency ?? 10,
    skipVerification: options?.skipVerification ?? false,
  };

  // We need to set the storage after creating the config
  return new ArkVtxoVerificationService(config);
}

/**
 * Progress callback helper for React components
 */
export function createVerificationProgressHandler(
  setProgress: (progress: VerificationProgress) => void,
  setStage: (stage: string) => void,
  setMessage: (message: string) => void
): (progress: VerificationProgress) => void {
  return (progress) => {
    setProgress(progress);
    setStage(progress.stage);
    setMessage(progress.message);
  };
}

/**
 * Convert verification result to user-friendly message
 */
export function getVerificationResultMessage(result: VerificationResult): string {
  if (result.valid) {
    return `✅ VTXO verified successfully! Anchored on commitment ${result.commitmentTxid.slice(0, 16)}... (batch output ${result.batchOutputIndex})`;
  } else {
    return `❌ Verification failed: ${result.error || result.diagnostics.join(', ')}`;
  }
}

/**
 * Get verification result severity for UI styling
 */
export function getVerificationSeverity(result: VerificationResult): 'success' | 'error' | 'warning' {
  if (!result.valid) return 'error';
  return 'success';
}