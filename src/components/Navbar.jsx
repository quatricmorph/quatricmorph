import React from 'react'
import logo from '../assets/logo.png'

export default function Navbar(){
  return (
    <nav className="w-full py-4 px-6 flex items-center justify-between z-30">
      <div className="flex items-center gap-3">
        <img src={logo} alt="QuatricMorph" className="w-10 h-10 object-contain drop-shadow-[0_8px_24px_rgba(15,74,120,0.24)]" />
        <span className="font-medium tracking-wider">QuatricMorph</span>
      </div>
      <ul className="hidden md:flex gap-6 text-qm-silver/80">
        <li className="hover:text-white transition">Home</li>
        <li className="hover:text-white transition">About</li>
        <li className="hover:text-white transition">Contact</li>
      </ul>
      <button className="ml-4 bg-gradient-to-r from-qm-metal/80 to-qm-deep/60 px-4 py-2 rounded-lg text-sm shadow-md hidden md:inline-block">Get Started</button>
    </nav>
  )
}