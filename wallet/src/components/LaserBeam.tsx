import { motion } from 'framer-motion'
import './LaserBeam.css'

interface LaserBeamProps {
  active: boolean
}

export default function LaserBeam({ active }: LaserBeamProps) {
  if (!active) return null
  
  return (
    <div className='laser-container'>
      <motion.div 
        className='laser-beam orange-laser-1'
        initial={{ x: '-100%', opacity: 0 }}
        animate={{ x: '150%', opacity: [0, 1, 1, 0] }}
        transition={{ duration: 1.2, ease: 'easeInOut', repeat: Infinity, repeatDelay: 0.5 }}
      />
      <motion.div 
        className='laser-beam orange-laser-2'
        initial={{ y: '-100%', opacity: 0 }}
        animate={{ y: '150%', opacity: [0, 1, 1, 0] }}
        transition={{ duration: 0.9, ease: 'easeInOut', repeat: Infinity, repeatDelay: 0.2 }}
      />
      <motion.div 
        className='laser-beam orange-laser-3'
        initial={{ x: '100%', y: '100%', opacity: 0, rotate: 45 }}
        animate={{ x: '-150%', y: '-150%', opacity: [0, 1, 1, 0] }}
        transition={{ duration: 1.5, ease: 'easeOut', repeat: Infinity, repeatDelay: 0.8 }}
      />
    </div>
  )
}
