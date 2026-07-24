import init, {
  quote,
  generate_identity_keypair,
  sign_profile_json
} from 'satspath-wasm'

let initialized = false
let initPromise: Promise<void> | null = null

/**
 * Initializes the SatsPath WASM module if it hasn't been initialized yet.
 */
export async function ensureSatsPathInitialized(): Promise<void> {
  if (initialized) return
  if (initPromise) return initPromise

  initPromise = (async () => {
    try {
      // Initialize with the static wasm file in the public directory
      await init('/satspath_wasm_bg.wasm')
      initialized = true
    } catch (e) {
      console.error('Failed to initialize SatsPath WASM', e)
      throw e
    }
  })()

  return initPromise
}

export interface RouteResult {
  methodType: string
  payload: string
  feeEstimate: number
}

/**
 * Resolves an alias (e.g. "rodrigo@satspath.dev") and routes the payment
 * using the SatsPath engine, returning the optimal method and payload.
 *
 * @param alias The SatsPath alias to resolve
 * @param amountSats The payment amount in satoshis
 * @returns The best route result or null if no route found
 */
export async function routePayment(alias: string, amountSats: number): Promise<RouteResult | null> {
  await ensureSatsPathInitialized()

  // Mock alias for local testing without Nostr relay broadcasts
  if (alias.toLowerCase() === 'chello@dev.idk') {
    return {
      methodType: 'Lightning',
      payload: 'chello@arkade.computer',
      feeEstimate: 0
    }
  }

  try {
    const quoteResponse: any = await quote(alias, amountSats)
    if (!quoteResponse) {
        return null
    }
    
    // QuoteResponse enum is returned as a JS object.
    // QuoteResponse::Ok matches an object with 'Ok' key, OR it might be serialized directly.
    // Let's assume serde_wasm_bindgen returns an object with a field "Ok" or we just check fields
    // Actually, serde_wasm_bindgen serializes enums as objects with the variant name as key if internally tagged,
    // or externally tagged `{ "Ok": { ... } }`. Let's assume externally tagged by default.
    let okData = quoteResponse.Ok || quoteResponse

    if (!okData || !okData.selected_method) {
      console.error('SatsPath Routing Error or no route:', quoteResponse)
      return null
    }

    const bestMethod = okData.selected_method
    
    // Convert Rust enum representation to something easier for Arkade
    let methodType = ''
    let payload = ''

    if (bestMethod.Lightning) {
        methodType = 'lightning'
        payload = bestMethod.Lightning.address || bestMethod.Lightning.bolt11 || bestMethod.Lightning.bolt12_offer || okData.qr
    } else if (bestMethod.Ark) {
        methodType = 'ark'
        payload = bestMethod.Ark.pubkey
    } else if (bestMethod.Onchain) {
        methodType = 'onchain'
        payload = bestMethod.Onchain.address || bestMethod.Onchain.silent_payment_code
    }

    return {
      methodType,
      payload,
      feeEstimate: okData.fee_sats || 0,
    }
  } catch (error) {
    console.error('SatsPath routePayment error:', error)
    throw error
  }
}

export async function buildSatsPathProfile(
  alias: string,
  lightningAddress: string,
  arkPubkey: string,
  onchainAddress: string
) {
  await ensureSatsPathInitialized()

  const keypair = generate_identity_keypair()
  // @ts-ignore - The WASM getter properties
  const pubkey = keypair.pubkey_hex
  // @ts-ignore
  const secretKey = keypair.secret_key_hex

  const profile = {
    alias,
    identity_pubkey: pubkey,
    methods: [
      {
        type: 'Lightning',
        label: 'Lightning Address',
        lightning_address: lightningAddress,
        lnurl: null,
        bolt12: null,
        receiver_pubkey: null
      },
      {
        type: 'Ark',
        label: 'Ark',
        server: 'https://arkade.computer', // Assuming default ASP
        pubkey: arkPubkey,
        vtxo_pointer: null,
        opaque_uri: null,
        proof: null,
        expires_at: null
      },
      {
        type: 'Onchain',
        label: 'Bitcoin (Mainnet)',
        network: 'Mainnet',
        address: onchainAddress,
        pubkey_hint: null,
        descriptor_hint: null
      }
    ],
    updated_at: Math.floor(Date.now() / 1000),
    expires_at: Math.floor(Date.now() / 1000) + (30 * 24 * 60 * 60), // 30 days
    sequence: 1,
    preferences: ['lightning', 'ark', 'onchain'],
    nonce: Array.from(crypto.getRandomValues(new Uint8Array(16)))
      .map(b => b.toString(16).padStart(2, '0')).join('')
  }

  const signedJson = sign_profile_json(JSON.stringify(profile), secretKey)
  
  return {
    secretKey,
    pubkey,
    signedJson
  }
}
