/**
 * SatsPath WASM Bridge for Arkade Wallet
 *
 * Responsibilities:
 *  - Initialize satspath-wasm (Rust compiled to WASM)
 *  - buildSatsPathProfile(): generate keypair, sign profile, encrypt secret key
 *  - publishProfileToNostr(): broadcast SignedPaymentProfile to Nostr (NIP-78)
 *  - routePayment(): resolve alias → verify → quote → return best rail + trust info
 *  - encryptSecretKey() / decryptSecretKey(): AES-GCM via Web Crypto API
 */

import init, {
  quote,
  generate_identity_keypair,
  sign_profile_json,
  generate_hybrid_identity_keypair,
  sign_hybrid_profile_json,
} from 'satspath-wasm'

export { generate_hybrid_identity_keypair }
import { finalizeEvent, SimplePool } from 'nostr-tools'
import { toXOnlyHex } from './keys'

// ─── WASM init ────────────────────────────────────────────────────────────────

let initialized = false
let initPromise: Promise<void> | null = null

export async function ensureSatsPathInitialized(): Promise<void> {
  if (initialized) return
  if (initPromise) return initPromise

  initPromise = (async () => {
    try {
      await init('/satspath_wasm_bg.wasm')
      initialized = true
    } catch (e) {
      console.error('[SatsPath] Failed to initialize WASM', e)
      throw e
    }
  })()

  return initPromise
}

// ─── Crypto: AES-GCM secret key encryption ────────────────────────────────────

/**
 * Derive an AES-GCM key from a password string using PBKDF2.
 * Used to protect the SatsPath identity secret key in LocalStorage.
 */
async function deriveAesKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const baseKey = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(password),
    'PBKDF2',
    false,
    ['deriveKey'],
  )
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt: salt as unknown as BufferSource, iterations: 200_000, hash: 'SHA-256' },
    baseKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  )
}

/**
 * Encrypt a hex secret key with a password.
 * Returns { ciphertext, iv, salt } as base64 strings.
 */
export async function encryptSecretKey(
  secretKeyHex: string,
  password: string,
): Promise<{ ciphertext: string; iv: string; salt: string }> {
  const salt = crypto.getRandomValues(new Uint8Array(16))
  const iv = crypto.getRandomValues(new Uint8Array(12))
  const key = await deriveAesKey(password, salt)
  const encrypted = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv },
    key,
    new TextEncoder().encode(secretKeyHex),
  )
  const toBase64 = (buf: ArrayBuffer | Uint8Array) =>
    btoa(String.fromCharCode(...new Uint8Array(buf instanceof Uint8Array ? buf.buffer : buf)))
  return {
    ciphertext: toBase64(encrypted),
    iv: toBase64(iv),
    salt: toBase64(salt),
  }
}

/**
 * Decrypt a previously encrypted secret key.
 * Returns the hex secret key string.
 */
export async function decryptSecretKey(
  encrypted: { ciphertext: string; iv: string; salt: string },
  password: string,
): Promise<string> {
  const fromBase64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0))
  const salt = fromBase64(encrypted.salt)
  const iv = fromBase64(encrypted.iv)
  const ciphertext = fromBase64(encrypted.ciphertext)
  const key = await deriveAesKey(password, salt)
  const decrypted = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ciphertext)
  return new TextDecoder().decode(decrypted)
}

// ─── Types ────────────────────────────────────────────────────────────────────

export type ResolverSource = 'local' | 'https' | 'bip353' | 'nostr' | 'p2p' | 'unknown'

export interface RouteResult {
  methodType: 'lightning' | 'ark' | 'onchain'
  /** The payment payload: Lightning Address / BOLT11, BIP-21 URI, or Ark URI */
  payload: string
  feeEstimate: number
  /** How this profile was resolved — shown as trust indicator in UI */
  resolverSource: ResolverSource
  /** Human-readable reason for the routing decision */
  routingReason: string
}

export interface SatsPathProfile {
  alias: string
  identity_pubkey: string
  methods: unknown[]
  updated_at: number
  expires_at: number
  sequence: number
  preferences: string[]
  nonce: string
  hybrid_pubkey: {
    classical_pubkey: string
    pqc_verification_key: string
    suite: string
  } | null
  pqc_required: boolean
}

export interface SignedProfile {
  profile: SatsPathProfile
  signature: string
}

// ─── Profile building ─────────────────────────────────────────────────────────

/**
 * Generate a fresh secp256k1 keypair, build and sign a SatsPath profile.
 * The secret key is returned in plain hex — caller must encrypt it before storing.
 */
export async function buildSatsPathProfile(
  alias: string,
  lightningAddress: string,
  arkPubkey: string,
  onchainAddress: string,
  /** Pass the existing sequence number to increment on update. Pass 1 for new profiles. */
  currentSequence = 1,
): Promise<{ secretKeyHex: string; pqcSeedHex: string; pubkeyHex: string; signedJson: string }> {
  await ensureSatsPathInitialized()

  const keypair = generate_hybrid_identity_keypair()
  // @ts-ignore — WASM getter properties
  const pubkeyHex: string = keypair.classical_pubkey_hex
  // @ts-ignore
  const secretKeyHex: string = keypair.classical_secret_key_hex
  // @ts-ignore
  const pqcVerificationKeyHex: string = keypair.pqc_verification_key_hex
  // @ts-ignore
  const pqcSeedHex: string = keypair.pqc_seed_hex

  const methods: unknown[] = []

  if (lightningAddress) {
    methods.push({
      type: 'Lightning',
      label: 'Lightning Address',
      lightning_address: lightningAddress,
      lnurl: null,
      bolt12: null,
      receiver_pubkey: null,
    })
  }

  if (arkPubkey) {
    methods.push({
      type: 'Ark',
      label: 'Ark (Arkade)',
      server: 'https://asp.arkade.bitcoin',
      pubkey: arkPubkey,
      vtxo_pointer: null,
      opaque_uri: null,
      proof: null,
      expires_at: null,
    })
  }

  if (onchainAddress) {
    methods.push({
      type: 'Onchain',
      label: 'Bitcoin (Mainnet)',
      network: 'Mainnet',
      address: onchainAddress,
      pubkey_hint: null,
      descriptor_hint: null,
    })
  }

  const now = Math.floor(Date.now() / 1000)
  const profile: SatsPathProfile = {
    alias,
    identity_pubkey: pubkeyHex,
    methods,
    updated_at: now,
    expires_at: now + 30 * 24 * 60 * 60, // 30 days
    sequence: currentSequence,
    preferences: ['lightning', 'ark', 'onchain'],
    nonce: Array.from(crypto.getRandomValues(new Uint8Array(16)))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join(''),
    hybrid_pubkey: {
      classical_pubkey: pubkeyHex,
      pqc_verification_key: pqcVerificationKeyHex,
      suite: 'ML-DSA-65-Schnorr',
    },
    pqc_required: true,
  }

  const signedJson = sign_hybrid_profile_json(JSON.stringify(profile), secretKeyHex, pqcSeedHex)

  return { secretKeyHex, pqcSeedHex, pubkeyHex, signedJson }
}

// ─── Nostr publishing ─────────────────────────────────────────────────────────

const SATSPATH_NOSTR_KIND = 30078 // NIP-78: application-specific data
const SATSPATH_NOSTR_TAG = 'satspath_profile'
const defaultRelays = [
  'wss://relay.damus.io',
  'wss://relay.primal.net',
  'wss://nostr.arkade.sh',
]

/**
 * Publish a SignedPaymentProfile to Nostr as a NIP-78 replaceable event (kind 30078).
 *
 * The event is signed with the SatsPath identity secret key (converted to x-only).
 * Resolvers with a Nostr backend will be able to discover and verify the profile.
 *
 * @param signedJson  The JSON string of the SignedPaymentProfile
 * @param secretKeyHex  The SatsPath identity secret key (hex, 32 bytes)
 * @returns true if at least one relay confirmed publication
 */
export async function publishProfileToNostr(
  signedJson: string,
  secretKeyHex: string,
): Promise<boolean> {
  try {
    const skBytes = Uint8Array.from(
      secretKeyHex.match(/.{2}/g)!.map((b) => parseInt(b, 16)),
    )

    const parsed: SignedProfile = JSON.parse(signedJson)
    const alias = parsed.profile.alias

    const event = {
      kind: SATSPATH_NOSTR_KIND,
      tags: [
        ['d', alias],
        ['t', SATSPATH_NOSTR_TAG],
        ['alt', `SatsPath payment profile for ${alias}`],
      ],
      created_at: Math.floor(Date.now() / 1000),
      content: signedJson,
    }

    const pool = new SimplePool()
    const signed = finalizeEvent(event, skBytes)

    const results = await Promise.allSettled(
      defaultRelays.map((relay) =>
        Promise.race([
          pool.publish([relay], signed),
          new Promise<void>((_, reject) =>
            setTimeout(() => reject(new Error('relay timeout')), 8000),
          ),
        ]),
      ),
    )

    pool.close(defaultRelays)

    const published = results.filter((r) => r.status === 'fulfilled').length
    console.log(`[SatsPath] Published to ${published}/${defaultRelays.length} relays`)
    return published > 0
  } catch (err) {
    console.error('[SatsPath] Nostr publish failed:', err)
    return false
  }
}

// ─── Payment routing ───────────────────────────────────────────────────────────

/**
 * Resolve a SatsPath alias, verify the profile, select the optimal payment rail,
 * and return a RouteResult with trust information.
 *
 * @param alias  e.g. "rodrigo@satspath.dev"
 * @param amountSats  Payment amount in satoshis
 */
export async function routePayment(
  alias: string,
  amountSats: number,
): Promise<RouteResult | null> {
  await ensureSatsPathInitialized()

  try {
    const quoteResponse: any = await quote(alias, amountSats)
    if (!quoteResponse) return null

    // serde_wasm_bindgen serializes QuoteResponse::Ok as { Ok: { ... } } externally tagged
    const okData = quoteResponse.Ok ?? quoteResponse

    if (!okData?.selected_method) {
      console.error('[SatsPath] No route or error:', quoteResponse)
      return null
    }

    const bestMethod = okData.selected_method
    const routingReason: string = okData.reason ?? ''
    const resolverSource = _extractResolverSource(okData)

    let methodType: RouteResult['methodType'] = 'lightning'
    let payload = ''

    if (bestMethod.Lightning) {
      methodType = 'lightning'
      const ln = bestMethod.Lightning
      // Prefer a real Lightning Address for the wallet's LNURL flow
      payload = ln.lightning_address ?? ln.bolt11 ?? ln.bolt12_offer ?? okData.qr ?? ''
    } else if (bestMethod.Ark) {
      methodType = 'ark'
      const ark = bestMethod.Ark
      // Prefer opaque URI (Arkade receive address), fallback to ark: URI
      payload =
        ark.opaque_uri ??
        `ark:${ark.pubkey}?server=${encodeURIComponent(ark.server ?? '')}`
    } else if (bestMethod.Onchain) {
      methodType = 'onchain'
      const chain = bestMethod.Onchain
      // BIP-21 URI with amount
      const address = chain.address ?? chain.silent_payment_pubkey ?? ''
      payload = address
        ? `bitcoin:${address}?amount=${(amountSats / 1e8).toFixed(8)}`
        : ''
    }

    if (!payload) {
      console.error('[SatsPath] Empty payload for method:', bestMethod)
      return null
    }

    return {
      methodType,
      payload,
      feeEstimate: okData.fee_sats ?? 0,
      resolverSource,
      routingReason,
    }
  } catch (error) {
    console.error('[SatsPath] routePayment error:', error)
    throw error
  }
}

/** Extract resolver source from QuoteResponse for trust display */
function _extractResolverSource(okData: any): ResolverSource {
  const src = (okData.resolver_source ?? okData.source ?? '').toLowerCase()
  if (src.includes('bip353') || src.includes('dns')) return 'bip353'
  if (src.includes('https') || src.includes('http')) return 'https'
  if (src.includes('nostr')) return 'nostr'
  if (src.includes('p2p') || src.includes('pear')) return 'p2p'
  if (src.includes('local')) return 'local'
  return 'unknown'
}
