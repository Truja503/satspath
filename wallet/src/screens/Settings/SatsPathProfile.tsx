import { useState, useContext, useEffect } from 'react'
import Header from './Header'
import Content from '../../components/Content'
import Padded from '../../components/Padded'
import FlexCol from '../../components/FlexCol'
import Button from '../../components/Button'
import Input from '../../components/Input'
import LaserBeam from '../../components/LaserBeam'
import Text from '../../components/Text'
import {
  buildSatsPathProfile,
  publishProfileToNostr,
  encryptSecretKey,
  decryptSecretKey,
} from '../../lib/satspath'
import {
  saveSatsPathProfileToStorage,
  readSatsPathProfileFromStorage,
  SatsPathLocalData,
} from '../../lib/storage'
import { WalletContext } from '../../providers/wallet'

type Step = 'idle' | 'generating' | 'publishing' | 'done' | 'error'

const LOCK_PASSWORD_KEY = 'satspath_lock_pw'

export default function SatsPathProfile() {
  const { wallet, svcWallet } = useContext(WalletContext)

  const [alias, setAlias] = useState('')
  const [lightningAddress, setLightningAddress] = useState('')
  const [step, setStep] = useState<Step>('idle')
  const [profileData, setProfileData] = useState<SatsPathLocalData | null>(null)
  const [publishStatus, setPublishStatus] = useState<'none' | 'ok' | 'fail'>('none')
  const [errorMsg, setErrorMsg] = useState('')

  useEffect(() => {
    const existing = readSatsPathProfileFromStorage()
    if (existing) setProfileData(existing)
  }, [])

  // ── Profile generation ────────────────────────────────────────────────────

  const handleGenerate = async () => {
    if (!alias) return
    setStep('generating')
    setErrorMsg('')

    try {
      // Small aesthetic delay for the laser animation
      await new Promise((r) => setTimeout(r, 1800))

      const arkPubkey = wallet?.pubkey ?? ''
      const onchainAddress = svcWallet ? await svcWallet.getAddress() : ''
      const sequence = (profileData?.sequence ?? 0) + 1

      const { secretKeyHex, pqcSeedHex, pubkeyHex, signedJson } = await buildSatsPathProfile(
        alias,
        lightningAddress,
        arkPubkey,
        onchainAddress,
        sequence,
      )

      // Derive a per-device encryption password from the wallet pubkey so we
      // don't need a separate user password prompt.
      const lockPassword = arkPubkey || pubkeyHex.slice(0, 32)
      const encryptedKey = await encryptSecretKey(secretKeyHex, lockPassword)
      const encryptedPqcSeed = await encryptSecretKey(pqcSeedHex, lockPassword)

      const newData: SatsPathLocalData = {
        alias,
        pubkey: pubkeyHex,
        encryptedKey,
        encryptedPqcSeed,
        signedJson,
        sequence,
      }

      saveSatsPathProfileToStorage(newData)
      setProfileData(newData)

      // ── Publish to Nostr ────────────────────────────────────────────────
      setStep('publishing')
      const ok = await publishProfileToNostr(signedJson, secretKeyHex)
      setPublishStatus(ok ? 'ok' : 'fail')
      setStep('done')
    } catch (e) {
      console.error('[SatsPath] Profile generation failed:', e)
      setErrorMsg(e instanceof Error ? e.message : 'Unknown error')
      setStep('error')
    }
  }

  const handleRepublish = async () => {
    if (!profileData) return
    setStep('publishing')
    setPublishStatus('none')

    try {
      const lockPassword = wallet?.pubkey || profileData.pubkey.slice(0, 32)
      const secretKeyHex = await decryptSecretKey(profileData.encryptedKey, lockPassword)
      const ok = await publishProfileToNostr(profileData.signedJson, secretKeyHex)
      setPublishStatus(ok ? 'ok' : 'fail')
    } catch (e) {
      console.error('[SatsPath] Re-publish failed:', e)
      setPublishStatus('fail')
    } finally {
      setStep('done')
    }
  }

  const handleReset = () => {
    setProfileData(null)
    setAlias('')
    setLightningAddress('')
    setPublishStatus('none')
    setStep('idle')
    setErrorMsg('')
  }

  const handleCopy = () => {
    if (profileData?.signedJson) {
      navigator.clipboard.writeText(profileData.signedJson)
    }
  }

  const isWorking = step === 'generating' || step === 'publishing'
  const stepLabel =
    step === 'generating'
      ? 'Generating identity...'
      : step === 'publishing'
        ? 'Broadcasting to Nostr...'
        : profileData
          ? 'Update & Re-sign Profile'
          : 'Generate & Sign Profile'

  // ── UI ────────────────────────────────────────────────────────────────────

  return (
    <>
      <Header text="SatsPath Identity" back />
      <LaserBeam active={isWorking} />
      <Content>
        <Padded>
          <FlexCol gap="1.5rem">
            {/* ── Existing profile ── */}
            {profileData ? (
              <FlexCol gap="1.2rem">
                <Text heading big>
                  Your SatsPath Profile
                </Text>

                {/* Profile card */}
                <div
                  style={{
                    background: 'linear-gradient(135deg, #1c1c2e 0%, #12122a 100%)',
                    border: '1px solid #3a3a5c',
                    padding: '1.2rem',
                    borderRadius: '16px',
                  }}
                >
                  <FlexCol gap="0.6rem">
                    <Text color="neutral-500" smaller>
                      Alias
                    </Text>
                    <Text big>{profileData.alias}</Text>

                    <Text color="neutral-500" smaller className="mt-1.5">
                      Identity Pubkey
                    </Text>
                    <div
                      style={{
                        wordBreak: 'break-all',
                        fontSize: '0.75rem',
                        color: '#a0aec0',
                        fontFamily: 'monospace',
                      }}
                    >
                      {profileData.pubkey}
                    </div>

                    <Text color="neutral-500" smaller className="mt-1.5">
                      Sequence #{profileData.sequence} · Key encrypted ✓
                    </Text>
                  </FlexCol>
                </div>

                {/* Publish status banner */}
                {publishStatus !== 'none' && (
                  <div
                    style={{
                      background: publishStatus === 'ok' ? '#0a2e1a' : '#2e0a0a',
                      border: `1px solid ${publishStatus === 'ok' ? '#22c55e' : '#ef4444'}`,
                      borderRadius: '12px',
                      padding: '0.8rem 1rem',
                    }}
                  >
                    <Text
                      color={publishStatus === 'ok' ? 'success' : 'error'}
                      smaller
                    >
                      {publishStatus === 'ok'
                        ? '✓ Profile broadcasted to Nostr relays. Resolvers can now find you.'
                        : '⚠ Could not reach Nostr relays. Copy & share the JSON manually.'}
                    </Text>
                  </div>
                )}

                <Button label="Broadcast to Nostr Again" onClick={handleRepublish} disabled={isWorking} />
                <Button label="Copy Signed Profile JSON" onClick={handleCopy} />
                <Button label="Reset Identity" onClick={handleReset} secondary />
              </FlexCol>
            ) : (
              /* ── New profile form ── */
              <FlexCol gap="1.5rem">
                <Text heading big>
                  Claim Your Alias
                </Text>
                <Text wrap color="neutral-400">
                  Register a SatsPath alias to receive Lightning, Ark, and On-chain payments.
                  Your identity key is generated locally and encrypted before storage.
                </Text>

                <FlexCol gap="0.5rem">
                  <Text>Alias</Text>
                  <Input
                    placeholder="e.g. satoshi@arkade.bitcoin"
                    value={alias}
                    onChange={(v) => setAlias(v)}
                    readOnly={isWorking}
                  />
                </FlexCol>

                <FlexCol gap="0.5rem">
                  <Text>Lightning Address (optional)</Text>
                  <Input
                    placeholder="e.g. satoshi@getalby.com"
                    value={lightningAddress}
                    onChange={(v) => setLightningAddress(v)}
                    readOnly={isWorking}
                  />
                </FlexCol>

                {step === 'error' && errorMsg && (
                  <div
                    style={{
                      background: '#2e0a0a',
                      border: '1px solid #ef4444',
                      borderRadius: '12px',
                      padding: '0.8rem 1rem',
                    }}
                  >
                    <Text color="error" smaller>
                      {errorMsg}
                    </Text>
                  </div>
                )}

                <Button
                  label={isWorking ? stepLabel : 'Generate & Sign Profile'}
                  onClick={handleGenerate}
                  loading={isWorking}
                  disabled={!alias || isWorking}
                  style={{
                    background: isWorking
                      ? 'linear-gradient(135deg, #f7931a, #ff6b35)'
                      : undefined,
                    boxShadow: isWorking ? '0 0 24px rgba(247,147,26,0.5)' : undefined,
                  }}
                />
              </FlexCol>
            )}
          </FlexCol>
        </Padded>
      </Content>
    </>
  )
}
