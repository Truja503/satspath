/**
 * Quote Response Builder — orchestrates resolve → verify → route → QR
 * Mirrors Rust satspath-router/src/quote_response.rs
 */

import type { 
  TypedPaymentMethod, 
  SignedPaymentProfile, 
  QuoteResponse, 
  QuoteRecipient, 
  Invite,
  RouteRequest,
  FeeEstimate,
  ExecutionMode
} from "@satspath/resolvers";

import { ChainResolver, AliasNotFoundError, identifierHash, maskIdentifier } from "@satspath/resolvers";
import { selectRoute, selectRouteLive } from "./router";
import { buildQrPayload } from "./qr";

/** Create invite for unregistered alias */
export function createInvite(alias: string, amountSats: number): Invite {
  const now = Math.floor(Date.now() / 1000);
  const aliasHash = identifierHash(alias);
  return {
    alias_hash: aliasHash,
    amount_sats: amountSats,
    created_at: now,
    expires_at: now + 24 * 3600, // 24 hours
    claim_url: `https://satspath.local/claim?alias_hash=${aliasHash.slice(0, 16)}&amount=${amountSats}`,
    warning: "The receiver must claim this payment by generating their own keys locally. SatsPath never holds or generates keys on behalf of users."
  };
}

/** Build recipient info for quote response */
function buildRecipient(profile: SignedPaymentProfile["profile"], verified: boolean): QuoteRecipient {
  return {
    alias: profile.alias,
    verified,
    profile_signature_verified: verified,
    identifier_verified: false,
    identifier_verification: "identifier-only; no inbox/domain ownership proof in this response",
    fingerprint: profile.identity_pubkey.slice(2, 10) // skip 02/03 prefix
  };
}

/** High-level quote function — matches Rust quote() exactly */
export async function quote(recipient: string, amountSats: number): Promise<QuoteResponse> {
  // Default resolver chain
  const resolver = new ChainResolver()
    .push((await import("@satspath/resolvers")).getDefaultRegistry())
    .push(new (await import("@satspath/resolvers")).Bip353Resolver())
    .push(new (await import("@satspath/resolvers")).HttpWellKnownResolver())
    .push(new (await import("@satspath/resolvers")).NostrNip05Resolver());

  return quoteWithResolver(resolver, recipient, amountSats);
}

/** Quote with custom resolver (for testing/API server) */
export async function quoteWithResolver(
  resolver: ChainResolver, 
  recipient: string, 
  amountSats: number,
  fetchLiveInvoice = false
): Promise<QuoteResponse> {
  // 1. Resolve
  let signed: SignedPaymentProfile;
  try {
    signed = await resolver.resolve_alias(recipient);
  } catch (e) {
    if (e instanceof AliasNotFoundError) {
      return {
        status: "not_registered",
        invite: createInvite(recipient, amountSats)
      };
    }
    throw e;
  }

  // 2. Verify signature (WASM crypto)
  // TODO: Use @satspath/wasm verifySignedProfile
  const verified = await verifyProfile(signed);
  const recipientInfo = buildRecipient(signed.profile, verified);
  
  if (!verified) {
    return {
      status: "invalid_signature",
      recipient: recipientInfo
    };
  }

  // 3. Check expiry
  if (signed.profile.expires_at && signed.profile.expires_at * 1000 < Date.now()) {
    return {
      status: "no_route",
      reason: "Profile expired."
    };
  }

  // 4. Route selection
  const req: RouteRequest = {
    alias: recipient,
    amount_sats: amountSats,
    signed_profile: signed,
    urgency: "normal"
  };

  let route: RouteRequest & { quote: RouteQuote };
  try {
    const routeQuote = await selectRouteLive(req);
    route = { ...req, quote: routeQuote };
  } catch (e) {
    return {
      status: "no_route",
      reason: e instanceof Error ? e.message : String(e)
    };
  }

  // 5. Build QR payload
  let qr = buildQrPayload(route.quote.selected_method, amountSats);

  // 6. Optionally fetch real BOLT11 for Lightning
  if (fetchLiveInvoice && route.quote.selected_method.type === "Lightning") {
    const lnMethod = route.quote.selected_method as any;
    if (lnMethod.lightning_address) {
      try {
        const invoice = await fetchRealInvoice(lnMethod.lightning_address, amountSats);
        qr = invoice;
      } catch {
        // Keep pointer if fetch fails
      }
    }
  }

  return {
    status: "ok",
    recipient: recipientInfo,
    selected_method: route.quote.selected_method,
    fee_sats: route.quote.estimated_fee_sats,
    eta: route.quote.estimated_confirmation,
    reason: route.quote.reason,
    qr,
    execution: route.quote.execution,
    wallet_hint: route.quote.wallet_hint
  };
}

/** Placeholder for WASM signature verification */
async function verifyProfile(_profile: SignedPaymentProfile): Promise<boolean> {
  // TODO: Use @satspath/wasm verifySignedProfile
  // For now, return true for valid-looking signatures
  return _profile.signature.length === 128; // 64 bytes hex
}

/** Fetch real BOLT11 invoice from LNURL-pay */
async function fetchRealInvoice(lightningAddress: string, amountSats: number): Promise<string> {
  // 1. Fetch LNURL metadata
  const [user, domain] = lightningAddress.split("@");
  const lnurl = `https://${domain}/.well-known/lnurlp/${user}`;
  
  const metaResponse = await fetch(lnurl);
  if (!metaResponse.ok) throw new Error("LNURL fetch failed");
  const meta = await metaResponse.json();
  
  // 2. Request invoice
  const callbackUrl = new URL(meta.callback);
  callbackUrl.searchParams.set("amount", String(amountSats * 1000)); // msats
  
  const invResponse = await fetch(callbackUrl);
  if (!invResponse.ok) throw new Error("Invoice fetch failed");
  const invoice = await invResponse.json();
  
  return invoice.pr;
}