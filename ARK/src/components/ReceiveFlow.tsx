import React, { useState, useEffect } from 'react';
import { SatspathService } from '../services/satspath';
import { SignedPaymentProfile, PaymentMethod } from '../types/satspath';

interface ReceiveFlowProps {
  onProfileUpdate?: (profile: SignedPaymentProfile) => void;
}

export const ReceiveFlow: React.FC<ReceiveFlowProps> = ({ onProfileUpdate }) => {
  const [profile, setProfile] = useState<SignedPaymentProfile | null>(null);
  const [methods, setMethods] = useState<PaymentMethod[]>([]);
  const [identityPubkey, setIdentityPubkey] = useState<string>('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [showAddMethod, setShowAddMethod] = useState(false);

  // Form fields for adding methods
  const [newMethod, setNewMethod] = useState<Partial<PaymentMethod>>({
    type: 'lightning'
  });

  const service = new SatspathService();

  useEffect(() => {
    loadProfile();
  }, []);

  const loadProfile = async () => {
    setIsLoading(true);
    try {
      // Try to load existing profile from localStorage
      const stored = localStorage.getItem('satspath:profile');
      if (stored) {
        const parsed = JSON.parse(stored);
        setProfile(parsed);
        setMethods(parsed.methods);
        setIdentityPubkey(parsed.identity_pubkey);
      } else {
        // Generate new identity keypair
        const identity = await SatspathService.generateIdentity();
        setIdentityPubkey(identity.pubkey);
      }
    } catch (e) {
      setError('Failed to load profile');
    } finally {
      setIsLoading(false);
    }
  };

  const addMethod = async () => {
    if (!newMethod.type) return;
    
    setIsLoading(true);
    try {
      const method: PaymentMethod = {
        type: newMethod.type,
        label: `${newMethod.type.charAt(0).toUpperCase() + newMethod.type.slice(1)} Method`,
        ...newMethod
      } as PaymentMethod;

      // Validate required fields
      if (method.type === 'lightning' && !method.lightning_address && !method.lnurl && !method.bolt12) {
        throw new Error('Lightning method requires lightning_address, lnurl, or bolt12');
      }
      if (method.type === 'onchain' && !method.address && !method.silent_payment_pubkey) {
        throw new Error('On-chain method requires address or silent_payment_pubkey');
      }
      if (method.type === 'ark' && (!method.server || !method.pubkey)) {
        throw new Error('Ark method requires server and pubkey');
      }

      const updatedMethods = [...methods, method];
      setMethods(updatedMethods);
      setNewMethod({ type: 'lightning' });
      setSuccess('Method added! Sign and publish your profile.');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to add method');
    } finally {
      setIsLoading(false);
    }
  };

  const removeMethod = (index: number) => {
    setMethods(methods.filter((_, i) => i !== index));
  };

  const signAndPublish = async () => {
    if (!profile && methods.length === 0) {
      setError('No profile or methods to publish');
      return;
    }

    setIsLoading(true);
    try {
      // Create new profile with current methods
      const newProfile: SignedPaymentProfile = {
        profile: {
          alias: profile?.profile?.alias || `user@${window.location.hostname}`,
          identity_pubkey: identityPubkey,
          methods: methods,
          updated_at: Math.floor(Date.now() / 1000),
          expires_at: Math.floor(Date.now() / 1000) + 30 * 24 * 60 * 60, // 30 days
          preferences: ['lightning', 'ark', 'onchain'],
          method_verifications: []
        },
        signature: '' // Will be filled by sign
      };

      // Sign the profile (this would use the private key from secure storage)
      // For now, we'll use the mock sign from SatspathService
      const signed = await SatspathService.signProfile(newProfile.profile, identityPubkey);
      
      // Save to localStorage
      localStorage.setItem('satspath:profile', JSON.stringify(signed));
      localStorage.setItem('satspath:identity', JSON.stringify({ pubkey: identityPubkey }));
      
      // Publish to well-known endpoint (if user controls domain)
      // await SatspathService.publishProfile(signed);
      
      setProfile(signed);
      setSuccess('Profile signed and saved! Use "Export" to share.');
      onProfileUpdate?.(signed);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to sign profile');
    } finally {
      setIsLoading(false);
    }
  };

  const exportProfile = () => {
    if (!profile) return;
    const json = JSON.stringify(profile, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `satspath-profile-${profile.profile.alias}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const importProfile = async (file: File) => {
    try {
      const text = await file.text();
      const parsed = JSON.parse(text);
      localStorage.setItem('satspath:profile', JSON.stringify(parsed));
      await loadProfile();
      setSuccess('Profile imported successfully');
    } catch (e) {
      setError('Invalid profile file');
    }
  };

  const getMethodIcon = (method: PaymentMethod) => {
    switch (method.type) {
      case 'lightning': return '⚡';
      case 'onchain': return '₿';
      case 'ark': return '🟣';
      default: return '📦';
    }
  };

  if (isLoading && !profile) {
    return <div style={{ padding: '20px', textAlign: 'center' }}>Loading profile...</div>;
  }

  return (
    <div style={{ maxWidth: '500px', margin: '0 auto', padding: '20px' }}>
      <h2 style={{ margin: '0 0 20px 0' }}>Receive Bitcoin</h2>

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

      {success && (
        <div style={{ 
          background: '#e8f5e9', 
          border: '1px solid #c8e6c9', 
          borderRadius: '8px', 
          padding: '12px', 
          marginBottom: '16px',
          color: '#2e7d32'
        }}>
          {success}
        </div>
      )}

      {!profile ? (
        <div style={{ textAlign: 'center', padding: '40px' }}>
          <p>No profile found. Generate an identity to start receiving.</p>
          <button 
            onClick={loadProfile}
            disabled={isLoading}
            style={{ padding: '12px 24px', background: '#007bff', color: 'white', border: 'none', borderRadius: '8px' }}
          >
            {isLoading ? 'Generating...' : 'Generate Identity'}
          </button>
        </div>
      ) : (
        <div>
          {/* Identity Section */}
          <div style={{ 
            marginBottom: '24px', 
            padding: '16px', 
            background: '#f8f9fa', 
            borderRadius: '8px' 
          }}>
            <h3 style={{ margin: '0 0 12px 0' }}>Your Identity</h3>
            <div style={{ fontSize: '14px', fontFamily: 'monospace', wordBreak: 'break-all' }}>
              {identityPubkey}
            </div>
            <p style={{ fontSize: '12px', color: '#666', margin: '8px 0 0 0' }}>
              This is your SatsPath identity public key. It's used to verify your payment profile.
            </p>
          </div>

          {/* Methods Section */}
          <div style={{ marginBottom: '24px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
              <h3 style={{ margin: 0 }}>Payment Methods</h3>
              <button 
                onClick={() => setShowAddMethod(true)}
                style={{ padding: '8px 16px', background: '#007bff', color: 'white', border: 'none', borderRadius: '6px' }}
              >
                + Add Method
              </button>
            </div>

            {methods.length === 0 ? (
              <p style={{ color: '#666', textAlign: 'center', padding: '20px' }}>
                No payment methods configured. Add one to start receiving.
              </p>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {methods.map((method, index) => (
                  <div key={index} style={{ 
                    display: 'flex', 
                    alignItems: 'center', 
                    gap: '12px', 
                    padding: '12px', 
                    background: '#fff', 
                    border: '1px solid #ddd', 
                    borderRadius: '8px' 
                  }}>
                    <span style={{ fontSize: '20px' }}>{getMethodIcon(method)}</span>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontWeight: '500' }}>{method.label}</div>
                      <div style={{ fontSize: '12px', color: '#666', fontFamily: 'monospace' }}>
                        {method.type === 'lightning' && (method.lightning_address || method.lnurl || method.bolt12 || 'Not configured')}
                        {method.type === 'onchain' && (method.address || method.silent_payment_pubkey || 'Not configured')}
                        {method.type === 'ark' && `${method.server} • ${method.pubkey.slice(0, 16)}...`}
                      </div>
                    </div>
                    <button 
                      onClick={() => removeMethod(index)}
                      style={{ padding: '6px 12px', background: '#dc3545', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer' }}
                    >
                      Remove
                    </button>
                  </div>
                ))}
            )}
          </div>

          {/* Add Method Modal */}
          {showAddMethod && (
            <div style={{ 
              position: 'fixed', 
              top: 0, 
              left: 0, 
              right: 0, 
              bottom: 0, 
              background: 'rgba(0,0,0,0.5)', 
              display: 'flex', 
              alignItems: 'center', 
              justifyContent: 'center',
              zIndex: 1000
            }}>
              <div style={{ 
                background: 'white', 
                padding: '24px', 
                borderRadius: '12px', 
                width: '90%', 
                maxWidth: '400px' 
              }}>
                <h3 style={{ margin: '0 0 16px 0' }}>Add Payment Method</h3>
                
                <div style={{ marginBottom: '16px' }}>
                  <label style={{ display: 'block', marginBottom: '8px' }}>Type</label>
                  <select
                    value={newMethod.type}
                    onChange={(e) => setNewMethod({ ...newMethod, type: e.target.value as any })}
                    style={{ width: '100%', padding: '10px', border: '1px solid #ddd', borderRadius: '6px' }}
                  >
                    <option value="lightning">Lightning (LNURL / Lightning Address / BOLT12)</option>
                    <option value="onchain">On-chain Bitcoin (Address / Silent Payments)</option>
                    <option value="ark">Ark VTXO</option>
                  </select>
                </div>

                {newMethod.type === 'lightning' && (
                  <div style={{ marginBottom: '16px' }}>
                    <label style={{ display: 'block', marginBottom: '8px' }}>Lightning Address (e.g., alice@getalby.com)</label>
                    <input
                      type="text"
                      value={newMethod.lightning_address || ''}
                      onChange={(e) => setNewMethod({ ...newMethod, lightning_address: e.target.value })}
                      placeholder="alice@getalby.com"
                      style={{ width: '100%', padding: '10px', border: '1px solid #ddd', borderRadius: '6px', boxSizing: 'border-box' }}
                    />
                  </div>
                )}

                {newMethod.type === 'onchain' && (
                  <div style={{ marginBottom: '16px' }}>
                    <label style={{ display: 'block', marginBottom: '8px' }}>Bitcoin Address</label>
                    <input
                      type="text"
                      value={newMethod.address || ''}
                      onChange={(e) => setNewMethod({ ...newMethod, address: e.target.value })}
                      placeholder="bc1q... or tb1q..."
                      style={{ width: '100%', padding: '10px', border: '1px solid #ddd', borderRadius: '6px', boxSizing: 'border-box' }}
                    />
                  </div>
                )}

                {newMethod.type === 'ark' && (
                  <>
                    <div style={{ marginBottom: '16px' }}>
                      <label style={{ display: 'block', marginBottom: '8px' }}>Ark Server URL</label>
                      <input
                        type="text"
                        value={newMethod.server || ''}
                        onChange={(e) => setNewMethod({ ...newMethod, server: e.target.value })}
                        placeholder="https://ark.example.com"
                        style={{ width: '100%', padding: '10px', border: '1px solid #ddd', borderRadius: '6px', boxSizing: 'border-box' }}
                      />
                    </div>
                    <div style={{ marginBottom: '16px' }}>
                      <label style={{ display: 'block', marginBottom: '8px' }}>Ark Pubkey (compressed hex)</label>
                      <input
                        type="text"
                        value={newMethod.pubkey || ''}
                        onChange={(e) => setNewMethod({ ...newMethod, pubkey: e.target.value })}
                        placeholder="02... (66 hex chars)"
                        style={{ width: '100%', padding: '10px', border: '1px solid #ddd', borderRadius: '6px', boxSizing: 'border-box' }}
                      />
                    </div>
                  </>
                )}

                <div style={{ display: 'flex', gap: '8px', marginTop: '16px' }}>
                  <button
                    onClick={() => setShowAddMethod(false)}
                    style={{ flex: 1, padding: '10px', background: '#6c757d', color: 'white', border: 'none', borderRadius: '6px' }}
                  >
                    Cancel
                  </button>
                  <button
                    onClick={addMethod}
                    disabled={isLoading}
                    style={{ flex: 1, padding: '10px', background: '#007bff', color: 'white', border: 'none', borderRadius: '6px' }}
                  >
                    {isLoading ? 'Adding...' : 'Add Method'}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Actions */}
          <div style={{ marginTop: '24px', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            <button
              onClick={signAndPublish}
              disabled={isLoading || methods.length === 0}
              style={{
                flex: 1,
                padding: '12px',
                background: methods.length > 0 ? '#28a745' : '#6c757d',
                color: 'white',
                border: 'none',
                borderRadius: '8px',
                fontWeight: '600',
                cursor: methods.length > 0 ? 'pointer' : 'not-allowed'
              }}
            >
              {isLoading ? 'Signing...' : 'Sign & Save Profile'}
            </button>
            
            <button
              onClick={exportProfile}
              disabled={!profile}
              style={{
                padding: '12px 24px',
                background: '#007bff',
                color: 'white',
                border: 'none',
                borderRadius: '8px',
                cursor: profile ? 'pointer' : 'not-allowed'
              }}
            >
              Export Profile
            </button>
          </div>

          {/* Import Profile */}
          <div style={{ marginTop: '24px', padding: '16px', background: '#f8f9fa', borderRadius: '8px' }}>
            <h4 style={{ margin: '0 0 12px 0' }}>Import Profile</h4>
            <input
              type="file"
              accept=".json"
              onChange={(e) => e.target.files?.[0] && importProfile(e.target.files[0])}
              style={{ display: 'block', marginBottom: '8px' }}
            />
            <p style={{ fontSize: '12px', color: '#666', margin: 0 }}>
              Import a previously exported SatsPath profile JSON file.
            </p>
          </div>
        </div>
      )}
    </div>
  );
};

export default ReceiveFlow;