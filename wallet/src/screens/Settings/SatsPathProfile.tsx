import { useState, useContext, useEffect } from 'react'
import Header from './Header'
import Content from '../../components/Content'
import Padded from '../../components/Padded'
import FlexCol from '../../components/FlexCol'
import Button from '../../components/Button'
import Input from '../../components/Input'
import LaserBeam from '../../components/LaserBeam'
import { buildSatsPathProfile } from '../../lib/satspath'
import { saveSatsPathProfileToStorage, readSatsPathProfileFromStorage, SatsPathLocalData } from '../../lib/storage'
import { WalletContext } from '../../providers/wallet'
import Text from '../../components/Text'

export default function SatsPathProfile() {
  const { wallet, svcWallet } = useContext(WalletContext)
  
  const [alias, setAlias] = useState('')
  const [lightningAddress, setLightningAddress] = useState('')
  const [isGenerating, setIsGenerating] = useState(false)
  const [profileData, setProfileData] = useState<SatsPathLocalData | null>(null)

  useEffect(() => {
    const existing = readSatsPathProfileFromStorage()
    if (existing) {
      setProfileData(existing)
    }
  }, [])

  const handleGenerate = async () => {
    if (!alias) return
    setIsGenerating(true)

    try {
      // Simulate network delay for the aesthetic laser effect
      await new Promise((resolve) => setTimeout(resolve, 2000))
      
      const arkPubkey = wallet.pubkey || ''
      const onchainAddress = svcWallet ? await svcWallet.getAddress() : ''

      const result = await buildSatsPathProfile(alias, lightningAddress, arkPubkey, onchainAddress)
      
      const newData: SatsPathLocalData = {
        alias,
        pubkey: result.pubkey,
        secretKey: result.secretKey,
        signedJson: result.signedJson
      }
      
      saveSatsPathProfileToStorage(newData)
      setProfileData(newData)
    } catch (e) {
      console.error('Failed to generate profile', e)
    } finally {
      setIsGenerating(false)
    }
  }

  const handleCopy = () => {
    if (profileData?.signedJson) {
      navigator.clipboard.writeText(profileData.signedJson)
      alert('Profile copied to clipboard!')
    }
  }

  return (
    <>
      <Header text='SatsPath Identity' back />
      <LaserBeam active={isGenerating} />
      <Content>
        <Padded>
          <FlexCol gap='1.5rem'>
            {profileData ? (
              <FlexCol gap='1rem'>
                <Text heading big>Your SatsPath Profile</Text>
                <div style={{ background: '#1c1c1e', padding: '1rem', borderRadius: '12px' }}>
                  <Text color='neutral-500'>Alias: {profileData.alias}</Text>
                  <div style={{ wordBreak: 'break-all', marginTop: '0.5rem', fontSize: '0.85rem', color: 'var(--neutral-500)' }}>
                    Identity Pubkey:<br /> {profileData.pubkey}
                  </div>
                </div>
                <Text wrap>Your signed payment profile is ready to be broadcasted to Nostr or your ASP.</Text>
                <Button label='Copy Signed Profile JSON' onClick={handleCopy} />
                <Button label='Reset Identity' onClick={() => setProfileData(null)} />
              </FlexCol>
            ) : (
              <FlexCol gap='1.5rem'>
                <Text heading big>Claim Your Alias</Text>
                <Text wrap>
                  Register a SatsPath alias to receive Lightning, Ark, and On-chain payments directly to your wallet.
                </Text>
                
                <FlexCol gap='0.5rem'>
                  <Text>Alias</Text>
                  <Input 
                    placeholder='e.g. satoshi@arkade.computer' 
                    value={alias} 
                    onChange={(v) => setAlias(v)} 
                    disabled={isGenerating}
                  />
                </FlexCol>

                <FlexCol gap='0.5rem'>
                  <Text>Lightning Address (Optional)</Text>
                  <Input 
                    placeholder='e.g. satoshi@getalby.com' 
                    value={lightningAddress} 
                    onChange={(v) => setLightningAddress(v)} 
                    disabled={isGenerating}
                  />
                </FlexCol>

                <Button 
                  label={isGenerating ? 'Generating Identity...' : 'Generate & Sign Profile'} 
                  onClick={handleGenerate} 
                  loading={isGenerating}
                  disabled={!alias || isGenerating}
                  style={{
                    background: isGenerating ? '#f7931a' : undefined,
                    boxShadow: isGenerating ? '0 0 20px #f7931a' : undefined
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
