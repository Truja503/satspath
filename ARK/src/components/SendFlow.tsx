// Arkade Wallet - Send Flow Component using SatsPath

import React, { useState, useCallback } from 'react';
import { SatspathService } from './satspath';

interface SendFlowProps {
  onPaymentComplete?: (result: { txid: string; method: string }) => void;
  onError?: (error: Error) => void;
}

export const SendFlow: React.FC<SendFlowProps> = ({ onPaymentComplete, onError }) => {
  const [recipient, setRecipient] = useState('');
  const [amountSats, setAmountSats] = useState('');
  const [memo, setMemo] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [quote, setQuote] = useState<any>(null);
  const [profile, setProfile] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const service = new SatspathService();

  const handleResolve = useCallback(async () => {
    if (!recipient.trim()) {
      setError('Enter a recipient alias');
      return;
    }

    setIsLoading(true);
    setError(null);
    setQuote(null);
    setProfile(null);

    try {
      const signedProfile = await service.resolveProfile(recipient.trim());
      setProfile(signedProfile.profile);

      // If amount is entered, get quote
      if (amountSats) {
        const amount = BigInt(amountSats);
        // Try WASM quote first, fallback to local routing
        try {
          const quoteResult = await service.getQuoteWASM(recipient.trim(), amount);
          setQuote(quoteResult);
        } catch (e) {
          // Fallback to local routing
          console.warn('WASM quote unavailable, using local routing');
          const feeEstimate = {
            fastest_fee: 10,
            half_hour_fee: 5,
            hour_fee: 3,
            economy_fee: 2,
            minimum_fee: 1
          };
          const routing = SatspathService.selectRoute(
            signedProfile.profile,
            parseInt(amountSats),
            feeEstimate
          );
          setQuote({
            selected_method: routing.method,
            fee_sats: routing.fee,
            eta: 'varies',
            reason: routing.reason,
            qr: '', // Will be built based on method
            execution: { type: 'ManualWallet' },
            wallet_hint: 'Use your preferred wallet to complete payment'
          });
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to resolve recipient');
    } finally {
      setIsLoading(false);
    }
  }, [recipient, amountSats]);

  const handleAmountChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setAmountSats(e.target.value);
    if (e.target.value && profile) {
      handleResolve(); // Re-resolve to get updated quote
    }
  };

  const handlePay = async () => {
    if (!quote || !profile) {
      setError('No quote available');
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const method = quote.selected_method;
      let paymentPayload = '';

      // Build payment payload based on method
      if (method.type === 'Lightning') {
        paymentPayload = method.lightning_address || method.lnurl || method.bolt12 || '';
      } else if (method.type === 'Onchain') {
        const target = method.silent_payment_pubkey || method.address || '';
        const btc = (BigInt(quote.fee_sats) / 100_000_000n).toString();
        paymentPayload = `bitcoin:${target}?amount=${btc}`;
      } else if (method.type === 'Ark') {
        paymentPayload = `ark:${method.pubkey}?server=${encodeURIComponent(method.server)}&amount=${quote.fee_sats}`;
      }

      // In a real app, this would open the appropriate wallet
      // For now, copy to clipboard
      await navigator.clipboard.writeText(paymentPayload);
      
      onPaymentComplete?.({
        txid: 'pending',
        method: quote.selected_method.type
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Payment failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="send-flow" style={{ maxWidth: '500px', margin: '0 auto', padding: '20px' }}>
      <h2 style={{ marginBottom: '20px' }}>Send Bitcoin</h2>

      {error && (
        <div style={{ 
          background: '#fee', 
          border: '1px solid #fcc', 
          borderRadius: '8px', 
          padding: '12px', 
          marginBottom: '16px',
          color: '#c00'
        }}>
          {error}
        </div>
      )}

      <div style={{ marginBottom: '16px' }}>
        <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
          Recipient (alias@domain)
        </label>
        <input
          type="text"
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          placeholder="alice@example.com"
          style={{ 
            width: '100%', 
            padding: '12px', 
            border: '1px solid #ddd', 
            borderRadius: '8px',
            fontSize: '16px',
            boxSizing: 'border-box'
          }}
          onKeyDown={(e) => e.key === 'Enter' && handleResolve()}
        />
      </div>

      <div style={{ marginBottom: '16px' }}>
        <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
          Amount (sats)
        </label>
        <input
          type="number"
          value={amountSats}
          onChange={handleAmountChange}
          placeholder="50000"
          min="1"
          style={{ 
            width: '100%', 
            padding: '12px', 
            border: '1px solid #ddd', 
            borderRadius: '8px',
            fontSize: '16px',
            boxSizing: 'border-box'
          }}
        />
      </div>

      <div style={{ marginBottom: '16px' }}>
        <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
          Memo (optional)
        </label>
        <input
          type="text"
          value={memo}
          onChange={(e) => setMemo(e.target.value)}
          placeholder="Coffee ☕"
          style={{ 
            width: '100%', 
            padding: '12px', 
            border: '1px solid #ddd', 
            borderRadius: '8px',
            fontSize: '16px',
            boxSizing: 'border-box'
          }}
        />
      </div>

      <button
        onClick={handleResolve}
        disabled={isLoading || !recipient.trim()}
        style={{
          width: '100%',
          padding: '14px',
          background: recipient.trim() ? '#007bff' : '#ccc',
          color: 'white',
          border: 'none',
          borderRadius: '8px',
          fontSize: '16px',
          fontWeight: '600',
          cursor: recipient.trim() ? 'pointer' : 'not-allowed'
        }}
      >
        {isLoading ? 'Resolving...' : 'Get Quote'}
      </button>

      {profile && (
        <div style={{ marginTop: '24px', padding: '16px', background: '#f8f9fa', borderRadius: '8px' }}>
          <h3 style={{ margin: '0 0 12px 0', fontSize: '16px' }}>
            Recipient: {SatspathService.maskIdentifier(profile.alias)}
          </h3>
          <div style={{ fontSize: '14px', color: '#666', marginBottom: '12px' }}>
            Identity: {SatspathService.fingerprintPubkey(profile.identity_pubkey)}
          </div>
          <div style={{ fontSize: '14px' }}>
            <strong>Available methods:</strong>
            <ul style={{ margin: '8px 0', paddingLeft: '20px' }}>
              {profile.methods.map((m: any, i: number) => (
                <li key={i} style={{ marginBottom: '4px' }}>
                  {m.type}: {m.label} 
                  {m.lightning_address && `(${m.lightning_address})`}
                  {m.address && `(${m.address})`}
                  {m.server && `(${m.server})`}
                </li>
              ))}
            </ul>
          </div>
        </div>
      )}

      {quote && (
        <div style={{ marginTop: '24px', padding: '16px', background: '#e8f5e9', borderRadius: '8px' }}>
          <h3 style={{ margin: '0 0 12px 0', color: '#2e7d32' }}>Payment Quote</h3>
          <div style={{ fontSize: '14px', lineHeight: '1.6' }}>
            <div><strong>Selected rail:</strong> {quote.selected_method?.type || 'Unknown'}</div>
            <div><strong>Label:</strong> {quote.selected_method?.label || 'N/A'}</div>
            <div><strong>Fee:</strong> {quote.fee_sats?.toLocaleString()} sats</div>
            <div><strong>ETA:</strong> {quote.eta}</div>
            <div><strong>Reason:</strong> {quote.reason}</div>
            {quote.wallet_hint && (
              <div style={{ marginTop: '8px', fontStyle: 'italic', color: '#555' }}>
                {quote.wallet_hint}
              </div>
            )}
          </div>
          {quote.qr && (
            <div style={{ marginTop: '12px', textAlign: 'center' }}>
              <strong>Payment QR:</strong>
              <div style={{ marginTop: '8px', fontFamily: 'monospace', fontSize: '11px', wordBreak: 'break-all' }}>
                {quote.qr}
              </div>
            </div>
          )}
        </div>
      )}

      {quote && !isLoading && (
        <button
          onClick={handlePay}
          style={{
            width: '100%',
            marginTop: '16px',
            padding: '14px',
            background: '#28a745',
            color: 'white',
            border: 'none',
            borderRadius: '8px',
            fontSize: '16px',
            fontWeight: '600',
            cursor: 'pointer'
          }}
        >
          Pay {amountSats} sats via {quote.selected_method?.type || 'selected rail'}
        </button>
      )}
    </div>
  );
};

export default SendFlow;