/**
 * Arkade Wallet - Main entry point
 * 
 * This is the main entry point for the Arkade wallet application.
 * It integrates SatsPath for payment routing and Arkade SDK for VTXO verification.
 * 
 * Architecture:
 * - SatsPath: Resolves aliases -> signed payment profiles -> route selection
 * - Arkade SDK: Verifies VTXO DAG -> sovereign exit ready
 * - Wallet UI: React components for Send/Receive flows
 */

export { SatspathService, SatspathServiceClass } from './services/satspath';
export type { 
  WasmPaymentMethod, 
  WasmPaymentProfile, 
  WasmSignedPaymentProfile,
  WasmQuoteResponse,
  WasmQuoteRecipient,
  WasmInvite,
  WasmFeeEstimate,
  WasmRouteRequest,
  WasmRouteQuote,
  WasmIdentity,
  SatspathConfig,
  WasmPaymentMethodType,
  WasmPaymentMethod,
  WasmPaymentProfile,
  WasmPaymentMethodType,
  WasmQuoteStatus
} from './services/satspath';

// Re-export Arkade SDK types
export * from './types/satspath';

// React components
export { SendFlow } from './components/SendFlow';
export { ReceiveFlow } from './components/ReceiveFlow';
export { App } from './App';

// Types
export type { 
  BitcoinNetwork,
  PaymentMethod,
  OnchainMethod,
  LightningMethod,
  ArkMethod,
  PaymentProfile,
  SignedPaymentProfile,
  KeyRotation,
  MethodVerification,
  PaymentRequest,
  Invite,
  InviteRecord,
  InviteStatus,
  FeeEstimate,
  RouteRequest,
  PaymentUrgency,
  FeeRateSnapshot,
  RouteQuote,
  SwapDirective,
  ExecutionMode,
  QuoteRecipient,
  QuoteResponse,
  QuoteStatus,
  PaymentRail,
  RouteCandidate,
  RouteDecision,
  RoutePreferences,
  WasmPaymentMethod,
  WasmPaymentProfile,
  WasmSignedPaymentProfile,
  WasmQuoteResponse,
  WasmQuoteRecipient,
  WasmInvite,
  WasmFeeEstimate,
  WasmRouteRequest,
  WasmRouteQuote,
  WasmIdentity,
  SatspathConfig,
  WasmPaymentMethodType
} from './types/satspath';