import React from 'react'
import logo from '../assets/logo.png'

export default function Hero(){
  return (
    <section className="z-20 flex flex-col items-center gap-6">
      <div className="flex items-center justify-center">
        <div className="bg-[linear-gradient(135deg,#0b2b45,rgba(15,74,120,0.6))] p-8 rounded-3xl shadow-[0_30px_60px_rgba(2,6,23,0.6)] border border-white/6 backdrop-blur-sm">
          <img src={logo} alt="QuatricMorph" className="w-36 h-36 md:w-48 md:h-48 object-contain" />
        </div>
      </div>

      <h1 className="mt-2 text-center text-2xl md:text-4xl font-bold tracking-wider" style={{fontFamily: 'Orbitron, sans-serif'}}>
        <span className="text-silver/70 text-qm-silver">Intelligence</span> <span className="text-white">in Motion</span>
      </h1>

      <p className="max-w-xl text-center text-qm-silver/60 mt-2">Self-organizing AI cubes forming, morphing and aligning — a visual metaphor for emergent intelligence.</p>
    </section>
  )
}