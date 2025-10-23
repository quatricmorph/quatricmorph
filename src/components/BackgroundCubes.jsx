import React from 'react'
import { motion } from 'framer-motion'

const cubeVariant = {
  hidden: { opacity: 0, scale: 0.6, rotateX: 10 },
  visible: i => ({
    opacity: 0.9,
    scale: 1,
    rotate: i * 20,
    x: [0, -10 + i * 6, 0],
    y: [0, -6 - i * 4, 0],
    transition: {
      duration: 6 + i,
      repeat: Infinity,
      repeatType: 'mirror',
      ease: 'easeInOut'
    }
  })
}

export default function BackgroundCubes(){
  const cubes = Array.from({length: 12}).map((_,i)=>i+1)
  return (
    <div aria-hidden className="absolute inset-0 -z-10 pointer-events-none">
      <div className="absolute inset-0 overflow-hidden">
        {cubes.map((i)=> (
          <motion.div
            key={i}
            custom={i}
            variants={cubeVariant}
            initial="hidden"
            animate="visible"
            className="cube will-change-transform absolute"
            style={{
              left: `${(i*9)%100}%`,
              top: `${(i*11)%100}%`,
              width: 80 + (i%4)*18,
              height: 80 + (i%4)*18,
              borderRadius: 8
            }}
          />
        ))}
      </div>
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,rgba(5,10,20,0.25),transparent 40%)] mix-blend-screen" />
    </div>
  )
}