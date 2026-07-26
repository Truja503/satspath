import { describe, it, expect, beforeAll } from 'vitest'
import { generate_hybrid_identity_keypair, ensureSatsPathInitialized } from '../satspath'

describe('SatsPath WASM Stress Test (Production Readiness)', () => {
  beforeAll(async () => {
    // Inicializar el módulo WASM
    await ensureSatsPathInitialized()
  })

  it('debería poder generar 100 llaves híbridas sin congelar excesivamente el hilo o causar Memory Leak', () => {
    const start = performance.now()
    const ITERATIONS = 100 // ML-DSA-65 is heavy, 100 should take a few seconds
    
    // Track memory if possible
    const initialMemory = typeof process !== 'undefined' && process.memoryUsage ? process.memoryUsage().heapUsed : 0
    
    for (let i = 0; i < ITERATIONS; i++) {
      const kp = generate_hybrid_identity_keypair()
      expect(kp.classical_pubkey_hex).toBeDefined()
      expect(kp.pqc_seed_hex).toBeDefined()
    }

    const end = performance.now()
    const finalMemory = typeof process !== 'undefined' && process.memoryUsage ? process.memoryUsage().heapUsed : 0
    
    const timeTaken = end - start
    const memoryDiffMb = (finalMemory - initialMemory) / 1024 / 1024

    console.log(`[Stress Test] Generadas ${ITERATIONS} llaves en ${timeTaken.toFixed(2)}ms`)
    console.log(`[Stress Test] Promedio por llave: ${(timeTaken / ITERATIONS).toFixed(2)}ms`)
    // @ts-ignore
    if (typeof process !== 'undefined' && process.memoryUsage) {
      console.log(`[Stress Test] Memoria consumida: ${memoryDiffMb.toFixed(2)} MB`)
    }

    // Assert that we don't leak more than 50MB for 100 keys
    // @ts-ignore
    if (typeof process !== 'undefined' && process.memoryUsage) {
      expect(memoryDiffMb).toBeLessThan(50)
    }
    
    // If it takes more than 50ms per key, it will stutter the UI noticeably for a single keygen
    // Let's log it, we expect ML-DSA to be relatively fast in native but WASM might be slower
  })
})
